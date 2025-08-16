use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
};

use aws_sdk_s3::Client as S3Client;

use actix_web::{
    delete, get, patch, post,
    web::{Data, Json, JsonConfig},
    App, HttpResponse, HttpServer, Responder,
};
use aws_sdk_s3::error::DisplayErrorContext;
use clap::Parser as _;
mod cornucopia;
use crate::cornucopia::queries::access::get_all_users_with_access;
use deadpool_postgres::Pool;
use futures::{stream::FuturesUnordered, StreamExt};
use pitsu_lib::{
    anyhow::{self, Result},
    decode_string_base64, encode_string_base64, AccessLevel, CreateRemoteRepository, FileUpload, Pitignore,
    RemoteRepository, RootFolder, SimpleRemoteRepository, ThisUser, UpdateRemoteRepository, User, UserWithAccess,
    VersionNumber,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    sync::Mutex,
};
use uuid::Uuid;

#[get("/")]
async fn root() -> impl Responder {
    HttpResponse::Ok().body(format!(
        "Planet51 Internet Transfer and Synchronization Utility Version {}",
        env!("CARGO_PKG_VERSION")
    ))
}

#[get("/api")]
async fn api() -> impl Responder {
    HttpResponse::Ok().body(format!(
        "Planet51 Internet Transfer and Synchronization Utility API Version {}",
        env!("CARGO_PKG_VERSION")
    ))
}

#[get("/api/{path:.*}")]
async fn api_catch_all(path: actix_web::web::Path<String>) -> impl Responder {
    let path = path.into_inner();
    log::debug!("API catch-all route hit with path: {path}");
    HttpResponse::NotFound().body(format!(
        "API endpoint not found: {path}. Please check the documentation for available endpoints."
    ))
}

#[get("/api/user")]
async fn get_self(req: actix_web::HttpRequest, pool: Data<Pool>) -> impl Responder {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };
    let owned_repositories: Vec<SimpleRemoteRepository> = match cornucopia::queries::repository::get_by_owner()
        .bind(&transaction, &user.uuid)
        .all()
        .await
    {
        Ok(repos) => {
            let mut new_repos: Vec<SimpleRemoteRepository> = Vec::with_capacity(repos.len());
            for repo in repos {
                let files: RootFolder = match serde_json::from_value(repo.file_hashes) {
                    Ok(files) => files,
                    Err(err) => {
                        log::error!("Failed to parse file hashes: {err}");
                        return HttpResponse::InternalServerError().body("Failed to parse file hashes");
                    }
                };
                new_repos.push(SimpleRemoteRepository {
                    uuid: repo.uuid,
                    name: repo.name.into(),
                    access_level: AccessLevel::Owner,
                    size: files.size(),
                    file_count: files.file_count(),
                });
            }
            new_repos
        }
        Err(err) => {
            log::error!("Failed to fetch owned repositories: {err}");
            return HttpResponse::InternalServerError().body("Failed to fetch owned repositories");
        }
    };
    let accessible_repositories: Vec<SimpleRemoteRepository> = match cornucopia::queries::access::get_by_user()
        .bind(&transaction, &user.uuid)
        .all()
        .await
    {
        Ok(access) => {
            let mut new_accessible_repos: Vec<SimpleRemoteRepository> = Vec::with_capacity(access.len());
            for access_entry in access {
                let repo = match cornucopia::queries::repository::get_by_uuid()
                    .bind(&transaction, &access_entry.repository_uuid)
                    .one()
                    .await
                {
                    Ok(repo) => repo,
                    Err(err) => {
                        log::error!("Failed to fetch repository: {err}");
                        return HttpResponse::InternalServerError().body("Failed to fetch repository");
                    }
                };
                let files: RootFolder = match serde_json::from_value(repo.file_hashes) {
                    Ok(files) => files,
                    Err(err) => {
                        log::error!("Failed to parse file hashes: {err}");
                        return HttpResponse::InternalServerError().body("Failed to parse file hashes");
                    }
                };
                new_accessible_repos.push(SimpleRemoteRepository {
                    uuid: repo.uuid,
                    name: repo.name.into(),
                    access_level: Into::<AccessLevel>::into(access_entry.access_level),
                    size: files.size(),
                    file_count: files.file_count(),
                });
            }
            new_accessible_repos
        }
        Err(err) => {
            log::error!("Failed to fetch accessible repositories: {err}");
            return HttpResponse::InternalServerError().body("Failed to fetch accessible repositories");
        }
    };
    HttpResponse::Ok().json(ThisUser {
        user,
        owned_repositories,
        accessible_repositories,
    })
}

#[get("/api/user/{uuid}")]
async fn get_other(
    req: actix_web::HttpRequest,
    uuid: actix_web::web::Path<uuid::Uuid>,
    pool: Data<Pool>,
) -> impl Responder {
    let pool = pool.into_inner();
    let _user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let uuid = uuid.into_inner();
    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    match cornucopia::queries::user::get_by_uuid()
        .bind(&transaction, &uuid)
        .one()
        .await
    {
        Ok(user) => HttpResponse::Ok().json(User {
            uuid: user.uuid,
            username: user.username.into(),
        }),
        Err(err) => {
            log::error!("Failed to fetch user: {err}");
            HttpResponse::InternalServerError().body("Failed to fetch user")
        }
    }
}

#[get("/api/users")]
async fn get_all_users(pool: Data<Pool>) -> impl Responder {
    let pool = pool.into_inner();
    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    match cornucopia::queries::user::get_all().bind(&transaction).all().await {
        Ok(users) => {
            let mut user_list: Vec<User> = Vec::with_capacity(users.len());
            for user in users {
                user_list.push(User {
                    uuid: user.uuid,
                    username: user.username.into(),
                });
            }
            HttpResponse::Ok().json(user_list)
        }
        Err(err) => {
            log::error!("Failed to fetch users: {err}");
            HttpResponse::InternalServerError().body("Failed to fetch users")
        }
    }
}

#[get("/{uuid}")]
async fn repository(
    req: actix_web::HttpRequest,
    uuid: actix_web::web::Path<uuid::Uuid>,
    pool: Data<Pool>,
) -> impl Responder {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let uuid = uuid.into_inner();

    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };

    if access_level == AccessLevel::None {
        log::warn!("User {} does not have access to repository {}", user.username, uuid);
        return HttpResponse::Forbidden().body("Access denied");
    }

    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    match cornucopia::queries::repository::get_by_uuid()
        .bind(&transaction, &uuid)
        .one()
        .await
    {
        Ok(repo) => {
            let files: RootFolder = match serde_json::from_value(repo.file_hashes) {
                Ok(files) => files,
                Err(err) => {
                    log::error!("Failed to parse file hashes: {err}");
                    return HttpResponse::InternalServerError().body("Failed to parse file hashes");
                }
            };

            match get_all_users_with_access().bind(&transaction, &uuid).all().await {
                Ok(users) => {
                    let pitignore = Pitignore::from_repository(
                        format!(
                            "{}/{}/",
                            std::env::var("ROOT_FOLDER").unwrap_or_else(|_| "repositories".to_string()),
                            repo.uuid
                        )
                        .into(),
                    )
                    .unwrap_or_default();
                    HttpResponse::Ok().json(RemoteRepository {
                        pitignore,
                        uuid: repo.uuid,
                        name: repo.name.into(),
                        access_level,
                        size: files.size(),
                        file_count: files.file_count(),
                        files,
                        users: users
                            .into_iter()
                            .map(|user| UserWithAccess {
                                user: User {
                                    uuid: user.user_uuid,
                                    username: user.username.into(),
                                },
                                access_level: Into::<AccessLevel>::into(user.access_level),
                            })
                            .collect(),
                    })
                }
                Err(err) => {
                    log::error!("Failed to fetch users with access: {err}");
                    HttpResponse::InternalServerError().body("Failed to fetch users with access")
                }
            }
        }
        Err(err) => {
            log::debug!("Failed to fetch repository: {err}");
            HttpResponse::InternalServerError().body("Failed to fetch repository")
        }
    }
}

#[patch("/{uuid}")]
async fn repository_update(
    req: actix_web::HttpRequest,
    uuid: actix_web::web::Path<uuid::Uuid>,
    pool: Data<Pool>,
    body: actix_web::web::Json<UpdateRemoteRepository>,
) -> impl Responder {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let uuid = uuid.into_inner();

    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };

    if access_level < AccessLevel::Owner {
        log::warn!(
            "User {} does not have admin access to repository {}",
            user.username,
            uuid
        );
        return HttpResponse::Forbidden().body("Access denied");
    }

    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    match cornucopia::queries::repository::update_metadata_by_uuid()
        .bind(&transaction, &&*body.name, &uuid)
        .one()
        .await
    {
        Ok(_) => {
            if let Err(err) = transaction.commit().await {
                log::error!("Failed to commit transaction: {err}");
                return HttpResponse::InternalServerError().body("Failed to commit changes");
            }
            HttpResponse::Ok().body("Repository updated successfully")
        }
        Err(err) => {
            log::error!("Failed to update repository: {err}");
            transaction.rollback().await.ok();
            HttpResponse::InternalServerError().body("Failed to update repository")
        }
    }
}

#[get("/{uuid}/.pit/access")]
async fn get_users_with_access(
    req: actix_web::HttpRequest,
    uuid: actix_web::web::Path<uuid::Uuid>,
    pool: Data<Pool>,
) -> impl Responder {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let uuid = uuid.into_inner();

    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };

    if access_level < AccessLevel::Owner {
        log::warn!(
            "User {} does not have admin access to repository {}",
            user.username,
            uuid
        );
        return HttpResponse::Forbidden().body("Access denied");
    }

    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    match cornucopia::queries::access::get_all_users_with_access()
        .bind(&transaction, &uuid)
        .all()
        .await
    {
        Ok(access_list) => {
            let mut users_with_access: Vec<UserWithAccess> = Vec::with_capacity(access_list.len());
            for access in access_list {
                // let Ok(access_level) = AccessLevel::try_from(access.access_level.as_str()) else {
                //     log::error!(
                //         "Invalid access level in database: {} for user {}",
                //         access.access_level,
                //         access.user_uuid
                //     );
                //     continue;
                // };
                let access_level = Into::<AccessLevel>::into(access.access_level);
                users_with_access.push(UserWithAccess {
                    user: User {
                        uuid: access.user_uuid,
                        username: access.username.into(),
                    },
                    access_level,
                });
            }
            HttpResponse::Ok().json(users_with_access)
        }
        Err(err) => {
            log::error!("Failed to fetch users with access: {err}");
            HttpResponse::InternalServerError().body("Failed to fetch users with access")
        }
    }
}

#[post("/{uuid}/.pit/user/access")]
async fn set_access_level(
    req: actix_web::HttpRequest,
    uuid: actix_web::web::Path<uuid::Uuid>,
    pool: Data<Pool>,
    body: actix_web::web::Json<UserWithAccess>,
) -> impl Responder {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let uuid = uuid.into_inner();

    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };

    if access_level < AccessLevel::Admin {
        log::warn!(
            "User {} does not have admin access to repository {}",
            user.username,
            uuid
        );
        return HttpResponse::Forbidden().body("Access denied");
    }

    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    if let Err(err) = cornucopia::queries::access::create_or_update()
        .bind(&transaction, &uuid, &body.user.uuid, &body.access_level.into())
        .await
    {
        log::error!("Failed to update access level: {err}");
        return HttpResponse::InternalServerError().body("Failed to update access level");
    }

    match transaction.commit().await {
        Ok(_) => HttpResponse::Ok().body("Access level updated successfully"),
        Err(err) => {
            log::error!("Failed to commit transaction: {err}");
            HttpResponse::InternalServerError().body("Failed to commit changes")
        }
    }
}

#[delete("/{uuid}/.pit/user/access")]
async fn remove_user_access(
    req: actix_web::HttpRequest,
    uuid: actix_web::web::Path<uuid::Uuid>,
    pool: Data<Pool>,
    body: actix_web::web::Json<Uuid>,
) -> impl Responder {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let uuid = uuid.into_inner();

    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };

    if access_level < AccessLevel::Admin {
        log::warn!(
            "User {} does not have admin access to repository {}",
            user.username,
            uuid
        );
        return HttpResponse::Forbidden().body("Access denied");
    }

    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    if let Err(err) = cornucopia::queries::access::delete_by_user_uuid_and_repository_uuid()
        .bind(&transaction, &body.0, &uuid)
        .one()
        .await
    {
        log::error!("Failed to remove access level: {err}");
        return HttpResponse::InternalServerError().body("Failed to remove access level");
    }
    match transaction.commit().await {
        Ok(_) => HttpResponse::Ok().body("Access level removed successfully"),
        Err(err) => {
            log::error!("Failed to commit transaction: {err}");
            HttpResponse::InternalServerError().body("Failed to commit changes")
        }
    }
}

#[get("{uuid}/{path:.*}")]
async fn repository_path(
    req: actix_web::HttpRequest,
    path_stuff: actix_web::web::Path<(uuid::Uuid, String)>,
    pool: Data<Pool>,
) -> impl Responder {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let (uuid, path) = path_stuff.into_inner();

    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };

    if access_level == AccessLevel::None {
        log::warn!("User {} does not have access to repository {}", user.username, uuid);
        return HttpResponse::Forbidden().body("Access denied");
    }

    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    let repo = match cornucopia::queries::repository::get_by_uuid()
        .bind(&transaction, &uuid)
        .one()
        .await
    {
        Ok(repo) => repo,
        Err(err) => {
            log::debug!("Failed to fetch repository: {err}");
            return HttpResponse::InternalServerError().body("Failed to fetch repository");
        }
    };
    let full_path = format!(
        "{}/{}/{}",
        std::env::var("ROOT_FOLDER").unwrap_or_else(|_| "repositories".to_string()),
        repo.uuid,
        path
    );
    log::debug!("Full path to file: {full_path}");

    if std::path::Path::new(&full_path).exists() {
        if std::path::Path::new(&full_path).is_dir() {
            log::debug!("Path is a directory, returning index");
            let root_folder: RootFolder = match serde_json::from_value(repo.file_hashes) {
                Ok(folder) => folder,
                Err(err) => {
                    log::error!("Failed to parse file hashes: {err}");
                    return HttpResponse::InternalServerError().body("Failed to parse file hashes");
                }
            };
            match root_folder.index_through(&path) {
                Ok(index) => HttpResponse::Ok().json(index),
                Err(err) => {
                    log::error!("Failed to index folder: {err}");
                    HttpResponse::InternalServerError().body("Failed to index folder")
                }
            }
        } else {
            match cornucopia::queries::files::get()
                .bind(&transaction, &path, &repo.uuid)
                .opt()
                .await
            {
                Ok(Some(file)) => {
                    let location = format!(
                        "https://{}.s3.{}.amazonaws.com/{}",
                        std::env!("AWS_BUCKET_NAME"),
                        std::env!("AWS_REGION"),
                        file.aws_s3_object_key
                    );
                    match HttpResponse::TemporaryRedirect()
                        .append_header(("Location", location.as_str()))
                        .await
                    {
                        Ok(response) => {
                            log::debug!("Redirecting to {location}");
                            response
                        }
                        Err(e) => {
                            log::error!("Failed to create redirect response: {e}");
                            // Fallback to serving the file from disk
                            match actix_files::NamedFile::open(full_path) {
                                Ok(file) => file.into_response(&req),
                                Err(err) => {
                                    log::error!("Failed to open file: {err}");
                                    HttpResponse::InternalServerError().body("File not found")
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    // Fallback to serving the file from disk
                    log::warn!("File not found in database, serving from disk: {full_path}");
                    match actix_files::NamedFile::open(full_path) {
                        Ok(file) => file.into_response(&req),
                        Err(err) => {
                            log::error!("Failed to open file: {err}");
                            HttpResponse::InternalServerError().body("File not found")
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to fetch file: {e}");
                    HttpResponse::InternalServerError().body("Failed to fetch file")
                }
            }
        }
    } else {
        HttpResponse::NotFound().body("File not found")
    }
}

#[post("{uuid}/.pit/upload")]
async fn upload_file(
    req: actix_web::HttpRequest,
    uuid: actix_web::web::Path<uuid::Uuid>,
    pool: Data<Pool>,
    client: Data<S3Client>,
    mut body: Json<FileUpload>,
) -> impl Responder {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };

    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };
    if access_level < AccessLevel::Write {
        log::warn!(
            "User {} does not have write access to repository {}",
            user.username,
            uuid
        );
        return HttpResponse::Forbidden().body("Access denied");
    }
    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    let repo = match cornucopia::queries::repository::get_by_uuid()
        .bind(&transaction, &uuid)
        .one()
        .await
    {
        Ok(repo) => repo,
        Err(err) => {
            log::debug!("Failed to fetch repository: {err}");
            return HttpResponse::InternalServerError().body("Failed to fetch repository");
        }
    };
    let root_path = std::env::var("ROOT_FOLDER").unwrap_or_else(|_| "repositories".to_string());
    let repo_path = format!("{}/{}", root_path, repo.uuid);
    let mut cleanup_paths = Vec::new();
    for file in &mut body.files {
        let raw_path = file.path.clone();
        let path = raw_path.trim_start_matches("/");
        let full_path = format!("{repo_path}/{path}");
        log::debug!("Full path to file: {full_path}");

        if let Some(parent) = std::path::Path::new(&full_path).parent() {
            if let Err(err) = tokio::fs::create_dir_all(parent).await {
                log::error!("Failed to create directory: {err}");
                return HttpResponse::InternalServerError().body("Failed to create directory");
            }
        }

        let bytes = match file.get_bytes() {
            Ok(bytes) => bytes,
            Err(err) => {
                log::error!("Failed to get file bytes: {err}");
                return HttpResponse::BadRequest().body("Invalid file data");
            }
        };

        if let Err(err) = tokio::fs::write(&full_path, &bytes).await {
            log::error!("Failed to write file: {err}");
            return HttpResponse::InternalServerError().body("Failed to write file");
        }
        // Add the file to the files table in the database and upload to S3
        {
            let bytestream = match aws_sdk_s3::primitives::ByteStream::from_path(PathBuf::from(&full_path)).await {
                Ok(stream) => stream,
                Err(err) => {
                    log::error!("Failed to create byte stream: {err}");
                    return HttpResponse::InternalServerError().body("Failed to create byte stream");
                }
            };
            let object_id = Uuid::new_v4();
            match client
                .put_object()
                .bucket(std::env!("AWS_BUCKET_NAME"))
                .key(object_id.to_string())
                .acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead)
                .body(bytestream)
                .send()
                .await
            {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Failed to upload file to S3: {err}");
                    return HttpResponse::InternalServerError().body("Failed to upload file to S3");
                }
            };

            match cornucopia::queries::files::create()
                .bind(&transaction, &repo.uuid, &path, &object_id.to_string())
                .one()
                .await
            {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Failed to insert file record: {err}");
                    return HttpResponse::InternalServerError().body("Failed to insert file record");
                }
            }
        }
        cleanup_paths.push(full_path.clone());
    }

    let root_folder = match RootFolder::ingest_folder(&repo_path.clone().into()) {
        Ok(folder) => folder,
        Err(err) => {
            log::error!("Failed to ingest folder: {err}");

            for path in cleanup_paths {
                let _ = tokio::fs::remove_file(&path).await;
            }
            return HttpResponse::InternalServerError().body("Failed to ingest folder");
        }
    };

    // .pitignore handling, delete any files that are in the .pitignore if it's just been uploaded
    let pitignore = Pitignore::from_repository(
        format!(
            "{}/{}/",
            std::env::var("ROOT_FOLDER").unwrap_or_else(|_| "repositories".to_string()),
            uuid
        )
        .into(),
    )
    .unwrap_or_default();

    for file in root_folder.files() {
        if pitignore.is_ignored(&file.full_path) {
            let full_path = format!("{}/{}/{}", root_path, repo.uuid, file.full_path);
            if let Err(err) = tokio::fs::remove_file(&full_path).await {
                log::error!("Failed to delete ignored file: {err}");
            } else {
                cleanup_paths.push(full_path);
            }
        }
    }

    let root_folder = match RootFolder::ingest_folder(&repo_path.into()) {
        Ok(folder) => folder,
        Err(err) => {
            log::error!("Failed to ingest folder: {err}");

            for path in cleanup_paths {
                let _ = tokio::fs::remove_file(&path).await;
            }
            return HttpResponse::InternalServerError().body("Failed to ingest folder");
        }
    };

    let file_hashes = match serde_json::to_value(&root_folder) {
        Ok(value) => value,
        Err(err) => {
            log::error!("Failed to serialize file hashes: {err}");

            for path in cleanup_paths {
                let _ = tokio::fs::remove_file(&path).await;
            }
            return HttpResponse::InternalServerError().body("Failed to serialize file hashes");
        }
    };
    match cornucopia::queries::repository::update_file_hashes_by_uuid()
        .bind(&transaction, &file_hashes, &repo.uuid)
        .one()
        .await
    {
        Ok(_) => {
            if let Err(err) = transaction.commit().await {
                log::error!("Failed to commit transaction: {err}");

                for path in cleanup_paths {
                    let _ = tokio::fs::remove_file(&path).await;
                }
                return HttpResponse::InternalServerError().body("Failed to commit changes");
            }
            HttpResponse::Ok().body("File uploaded successfully")
        }
        Err(err) => {
            log::error!("Failed to update file hashes: {err}");

            for path in cleanup_paths {
                let _ = tokio::fs::remove_file(&path).await;
            }
            transaction.rollback().await.ok();
            HttpResponse::InternalServerError().body("Failed to update file hashes")
        }
    }
}

#[delete("/{uuid}/{path:.*}")]
async fn delete_file(
    req: actix_web::HttpRequest,
    path_stuff: actix_web::web::Path<(uuid::Uuid, String)>,
    pool: Data<Pool>,
    client: Data<S3Client>,
) -> impl Responder {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let (uuid, path) = path_stuff.into_inner();

    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };
    if access_level < AccessLevel::Write {
        log::warn!(
            "User {} does not have write access to repository {}",
            user.username,
            uuid
        );
        return HttpResponse::Forbidden().body("Access denied");
    }
    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Transaction error");
        }
    };

    let repo = match cornucopia::queries::repository::get_by_uuid()
        .bind(&transaction, &uuid)
        .one()
        .await
    {
        Ok(repo) => repo,
        Err(err) => {
            log::debug!("Failed to fetch repository: {err}");
            return HttpResponse::InternalServerError().body("Failed to fetch repository");
        }
    };
    let root_path = std::env::var("ROOT_FOLDER").unwrap_or_else(|_| "repositories".to_string());
    let full_path = format!("{}/{}/{}", root_path, repo.uuid, path);
    log::debug!("Full path to file: {full_path}");

    let metadata = match tokio::fs::metadata(&full_path).await {
        Ok(meta) => meta,
        Err(err) => {
            log::debug!("Failed to get metadata for path: {err}");
            return HttpResponse::NotFound().body("File or directory not found");
        }
    };

    if metadata.is_dir() {
        if let Err(err) = tokio::fs::remove_dir_all(&full_path).await {
            log::error!("Failed to delete file or directory: {err}");
            return HttpResponse::InternalServerError().body("Failed to delete file or directory");
        }
    } else if metadata.is_file() {
        if let Err(err) = tokio::fs::remove_file(&full_path).await {
            log::error!("Failed to delete file: {err}");
            return HttpResponse::InternalServerError().body("Failed to delete file");
        }
    } else {
        log::debug!("Path is neither a file nor a directory");
        return HttpResponse::NotFound().body("File or directory not found");
    }

    // Delete file from table and delete from S3
    {
        match cornucopia::queries::files::delete()
            .bind(&transaction, &path, &repo.uuid)
            .one()
            .await
        {
            Ok(entry) => {
                // delete from S3
                tokio::spawn(async move {
                    if let Err(err) = client
                        .delete_object()
                        .bucket(env!("AWS_BUCKET_NAME"))
                        .key(entry.aws_s3_object_key)
                        .send()
                        .await
                        .map_err(|err| anyhow::anyhow!("Failed to delete file from S3: {err}"))
                    {
                        log::error!("Failed to delete S3 object: {err}");
                    }
                });
            }
            Err(err) => {
                log::error!("Failed to delete file record: {err}");
                return HttpResponse::InternalServerError().body("Failed to delete file record");
            }
        }
    }

    let root_folder = match RootFolder::ingest_folder(&format!("{}/{}", root_path, repo.uuid).into()) {
        Ok(folder) => folder,
        Err(err) => {
            log::error!("Failed to ingest folder: {err}");
            return HttpResponse::InternalServerError().body("Failed to ingest folder");
        }
    };
    let file_hashes = match serde_json::to_value(&root_folder) {
        Ok(value) => value,
        Err(err) => {
            log::error!("Failed to serialize file hashes: {err}");
            return HttpResponse::InternalServerError().body("Failed to serialize file hashes");
        }
    };
    match cornucopia::queries::repository::update_file_hashes_by_uuid()
        .bind(&transaction, &file_hashes, &repo.uuid)
        .one()
        .await
    {
        Ok(_) => {
            if let Err(err) = transaction.commit().await {
                log::error!("Failed to commit transaction: {err}");
                return HttpResponse::InternalServerError().body("Failed to commit changes");
            }
            HttpResponse::Ok().body("File deleted successfully")
        }
        Err(err) => {
            log::error!("Failed to update file hashes: {err}");
            transaction.rollback().await.ok();
            HttpResponse::InternalServerError().body("Failed to update file hashes")
        }
    }
}

#[post("/api/repository")]
async fn create_repository(
    req: actix_web::HttpRequest,
    pool: Data<Pool>,
    body: actix_web::web::Json<CreateRemoteRepository>,
) -> HttpResponse {
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let mut connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Failed to get database connection");
        }
    };
    let transaction = match connection.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            log::error!("Failed to start transaction: {err}");
            return HttpResponse::InternalServerError().body("Failed to start transaction");
        }
    };
    let res = crate::cornucopia::queries::repository::create()
        .bind(&transaction, &&*body.name, &user.uuid)
        .one()
        .await;
    match res {
        Ok(repo) => {
            transaction.commit().await.unwrap_or_else(|err| {
                log::error!("Failed to commit transaction: {err}");
            });
            HttpResponse::Created().json(RemoteRepository {
                pitignore: Pitignore::default(),
                uuid: repo.uuid,
                name: repo.name.into(),
                access_level: AccessLevel::Owner,
                size: 0,
                file_count: 0,
                files: RootFolder::default(),
                users: vec![UserWithAccess {
                    user: User {
                        uuid: user.uuid,
                        username: user.username,
                    },
                    access_level: AccessLevel::Owner,
                }],
            })
        }
        Err(err) => {
            log::error!("Failed to create repository: {err}");
            transaction.rollback().await.unwrap_or_else(|err| {
                log::error!("Failed to rollback transaction: {err}");
            });
            HttpResponse::InternalServerError().body("Failed to create repository")
        }
    }
}

#[get("/api/invite")]
async fn invite_user(
    req: actix_web::HttpRequest,
    pool: Data<Pool>,
    lock: Data<InviteLock>,
    query: actix_web::web::Query<InviteQuery>,
) -> impl Responder {
    let path = {
        // Acquire an invite lock to prevent concurrent invites (this builds the app with set env vars so we can't do multiple invites at once)
        #[allow(unused_variables, unused_mut)]
        let mut lock = lock.lock().await;
        let pool = pool.into_inner();
        let user_uuid = match query.extract() {
            Ok(code) => code,
            Err(err) => {
                log::error!("Failed to extract invite code: {err}");
                return HttpResponse::BadRequest().body("Invalid invite code format");
            }
        };
        let mut connection = match pool.get().await {
            Ok(conn) => conn,
            Err(err) => {
                log::error!("Failed to get database connection: {err}");
                return HttpResponse::InternalServerError().body("Database connection error");
            }
        };
        let transaction = match connection.transaction().await {
            Ok(tx) => tx,
            Err(err) => {
                log::error!("Failed to start transaction: {err}");
                return HttpResponse::InternalServerError().body("Transaction error");
            }
        };
        let user = match cornucopia::queries::user::get_by_uuid()
            .bind(&transaction, &user_uuid)
            .one()
            .await
        {
            Ok(user) => user,
            Err(err) => {
                log::error!("Failed to fetch user: {err}");
                return HttpResponse::InternalServerError().body("Failed to fetch user");
            }
        };
        // eventually expand this to build for the users OS, but for now just windows
        match build_executable(Some(Arc::from(user.api_key))).await {
            Ok(path) => path,
            Err(err) => {
                log::error!("Failed to build executable: {err}");
                return HttpResponse::InternalServerError().body("Failed to build executable");
            }
        }
    };
    // serve the executable file as a download
    let file = match actix_files::NamedFile::open(path) {
        Ok(file) => file,
        Err(err) => {
            log::error!("Failed to open executable file: {err}");
            return HttpResponse::InternalServerError().body("Failed to open invite file");
        }
    };
    file.into_response(&req)
}

#[get("/api/local/version")]
async fn get_local_version(req: actix_web::HttpRequest, pool: Data<Pool>) -> impl Responder {
    let pool = pool.into_inner();
    let _user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    // git pull in the Pitsu repository
    // let output = match std::process::Command::new("git")
    //     .arg("pull")
    //     .current_dir(env!("CARGO_MANIFEST_DIR"))
    //     .output()
    // {
    //     Ok(output) => output,
    //     Err(err) => {
    //         log::error!("Failed to run git pull: {err}");
    //         return HttpResponse::InternalServerError().body("Failed to update local version");
    //     }
    // };
    // if !output.status.success() {
    //     log::error!(
    //         "Git pull failed: {}",
    //         String::from_utf8_lossy(&output.stderr)
    //     );
    //     return HttpResponse::InternalServerError().body("Failed to update local version");
    // }
    // get the current git commit hash
    // let output = match std::process::Command::new("git")
    //     .arg("rev-parse")
    //     .arg("HEAD")
    //     .current_dir(env!("CARGO_MANIFEST_DIR"))
    //     .output()
    // {
    //     Ok(output) => output,
    //     Err(err) => {
    //         log::error!("Failed to get git commit hash: {err}");
    //         return HttpResponse::InternalServerError().body("Failed to get local version");
    //     }
    // };
    // let commit_hash = String::from_utf8_lossy(&output.stdout).to_string();
    let version_number = match get_client_version() {
        Ok(version) => version,
        Err(err) => {
            log::error!("Failed to get client version: {err}");
            return HttpResponse::InternalServerError().body("Failed to get local version");
        }
    };
    HttpResponse::Ok().json(version_number)
}

// like invite except checks via the user's bearer token rather than the invite code
#[get("/api/local/update")]
async fn get_latest_version(req: actix_web::HttpRequest, pool: Data<Pool>) -> impl Responder {
    let pool = pool.into_inner();
    let _user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    // git pull in the Pitsu repository
    // let output = match std::process::Command::new("git")
    //     .arg("pull")
    //     .current_dir(env!("CARGO_MANIFEST_DIR"))
    //     .output()
    // {
    //     Ok(output) => output,
    //     Err(err) => {
    //         log::error!("Failed to run git pull: {err}");
    //         return HttpResponse::InternalServerError().body("Failed to update local version");
    //     }
    // };
    // if !output.status.success() {
    //     log::error!(
    //         "Git pull failed: {}",
    //         String::from_utf8_lossy(&output.stderr)
    //     );
    //     return HttpResponse::InternalServerError().body("Failed to update local version");
    // }
    // build and serve the executable file
    let path = match build_executable(None).await {
        Ok(path) => path,
        Err(err) => {
            log::error!("Failed to build executable: {err}");
            return HttpResponse::InternalServerError().body("Failed to build executable");
        }
    };
    // serve the executable file as a download
    let file = match actix_files::NamedFile::open(path) {
        Ok(file) => file,
        Err(err) => {
            log::error!("Failed to open executable file: {err}");
            return HttpResponse::InternalServerError().body("Failed to open update file");
        }
    };
    // serve the executable file as a download
    file.into_response(&req)
}
struct InviteLock(Mutex<()>);

impl Deref for InviteLock {
    type Target = Mutex<()>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for InviteLock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(serde::Deserialize)]
struct InviteQuery {
    code: String,
}

impl InviteQuery {
    fn new(user_uuid: Uuid) -> Self {
        let encrypted_code = xor_cypher(format!(
            "{}|{}|{}",
            env!("INVITE_CODE_PREFIX"),
            user_uuid,
            env!("INVITE_CODE_SUFFIX")
        ));
        Self {
            code: encode_string_base64(&encrypted_code),
        }
    }
    fn extract(&self) -> Result<Uuid> {
        let decoded_code =
            decode_string_base64(&self.code).map_err(|_| anyhow::anyhow!("Failed to decode invite code"))?;
        let decrypted_code = xor_cypher(decoded_code);
        let parts: Vec<&str> = decrypted_code.split('|').collect();
        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Invalid invite code format"));
        }
        if parts[0] != env!("INVITE_CODE_PREFIX") || parts[2] != env!("INVITE_CODE_SUFFIX") {
            return Err(anyhow::anyhow!("Invalid invite code prefix or suffix"));
        }
        Uuid::parse_str(parts[1]).map_err(|_| anyhow::anyhow!("Invalid UUID in invite code"))
    }
}

fn xor_cypher(input: String) -> String {
    let key = env!("INVITE_CODE_ENCRYPTION_KEY");
    input
        .chars()
        .zip(key.chars().cycle())
        .map(|(c, k)| (c as u8 ^ k as u8) as char)
        .collect()
}

async fn exec(host: String, port: u16, pool: Pool, client: S3Client) -> Result<()> {
    std::env::set_var("SEQ_API_KEY", env!("REMOTE_SEQ_API_KEY"));
    if let Err(e) = datalust_logger::init("pitsu") {
        eprintln!("Failed to initialize logger: {e}");
        std::process::exit(1);
    };
    let json_cfg = JsonConfig::default().limit(pitsu_lib::MAX_UPLOAD_SIZE);
    HttpServer::new(move || {
        App::new()
            .app_data(json_cfg.clone())
            .app_data(Data::new(pool.clone()))
            .app_data(Data::new(InviteLock(Mutex::new(()))))
            .app_data(Data::new(client.clone()))
            .service(root)
            .service(set_access_level)
            .service(remove_user_access)
            .service(api)
            .service(invite_user)
            .service(get_local_version)
            .service(get_latest_version)
            .service(get_self)
            .service(get_other)
            .service(get_all_users)
            .service(get_users_with_access)
            .service(create_repository)
            .service(api_catch_all)
            .service(repository)
            .service(upload_file)
            .service(delete_file)
            .service(repository_path)
            .service(repository_update)
    })
    .bind((host, port))?
    .run()
    .await?;
    Ok(())
}

#[derive(clap::Parser)]
#[clap(name = "remote", version = env!("CARGO_PKG_VERSION"), author = "Ethan Conaway <you@willsh.art>", about = "Planet51 Internet Transfer and Synchronization Utility")]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}
#[derive(clap::Subcommand)]
enum Command {
    Run {
        #[clap(long, default_value = "8080", env = "PORT")]
        port: u16,
        #[clap(long, default_value = "0.0.0.0", env = "HOST")]
        host: String,
    },
    User {
        #[clap(subcommand)]
        user_command: UserCommand,
    },
    Repo {
        #[clap(subcommand)]
        repository_command: RepositoryCommand,
    },
}
#[derive(clap::Subcommand)]
enum UserCommand {
    Add { name: String },
    List,
    Remove { uuid: String },
    Invite { name: String },
}

#[derive(clap::Subcommand)]
enum RepositoryCommand {
    List,
    Sync {
        #[clap(subcommand)]
        stage: RepositorySyncStage,
    },
}

#[derive(clap::Subcommand, PartialEq, Eq)]
enum RepositorySyncStage {
    All,
    Hashes,
    Aws { repo: Option<Uuid> },
}

impl RepositorySyncStage {
    fn sync_hashes(&self) -> bool {
        matches!(self, RepositorySyncStage::All | RepositorySyncStage::Hashes)
    }

    fn sync_aws(&self) -> bool {
        matches!(self, RepositorySyncStage::All | RepositorySyncStage::Aws { .. })
    }

    fn aws_repo(&self) -> Option<Uuid> {
        if let RepositorySyncStage::Aws { repo } = self {
            *repo
        } else {
            None
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    let pool = create_pool().await.unwrap_or_else(|err| {
        log::error!("Failed to create database pool: {err}");
        std::process::exit(1);
    });
    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::v2025_01_17()).await;
    println!("Using AWS region: {:?}", aws_config.region());
    let s3_client = aws_sdk_s3::Client::new(&aws_config);

    match cli.command {
        Command::Run { port, host } => {
            exec(host, port, pool, s3_client).await.unwrap_or_else(|err| {
                log::error!("Failed to start server: {err}");
                std::process::exit(1);
            });
        }
        Command::User {
            user_command: UserCommand::Add { name },
        } => {
            println!("Adding user: {name}");
            let mut connection = pool.get().await.unwrap_or_else(|err| {
                log::error!("Failed to get database connection: {err}");
                std::process::exit(1);
            });
            let transaction = connection.transaction().await.unwrap_or_else(|err| {
                log::error!("Failed to start transaction: {err}");
                std::process::exit(1);
            });
            let user = match crate::cornucopia::queries::user::create()
                .bind(&transaction, &name)
                .one()
                .await
            {
                Ok(user) => user,
                Err(err) => {
                    log::error!("Failed to create user: {err}");
                    transaction.rollback().await.unwrap_or_else(|err| {
                        log::error!("Failed to rollback transaction: {err}");
                        std::process::exit(1);
                    });
                    std::process::exit(1);
                }
            };
            transaction.commit().await.unwrap_or_else(|err| {
                log::error!("Failed to commit transaction: {err}");
                std::process::exit(1);
            });
            println!("User {name} added successfully");
            let invite_code = InviteQuery::new(user.uuid);
            let invite_code_str = invite_code.code;
            println!(
                "Invite for user {name}: {}/api/invite?code={invite_code_str}",
                env!("PITSU_PUBLIC_URL")
            );
        }
        Command::User {
            user_command: UserCommand::List,
        } => {
            println!("Users:");
            let connection = pool.get().await.unwrap_or_else(|err| {
                log::error!("Failed to get database connection: {err}");
                std::process::exit(1);
            });
            let users = crate::cornucopia::queries::user::get_all()
                .bind(&connection)
                .all()
                .await
                .unwrap_or_else(|err| {
                    log::error!("Failed to list users: {err}");
                    std::process::exit(1);
                });
            for user in users.iter() {
                println!("- {} <{}>", user.username, user.uuid);
            }
            if users.is_empty() {
                println!("No users found.");
            }
        }
        Command::User {
            user_command: UserCommand::Remove { uuid },
        } => {
            println!("Removing user: {uuid}");
            let mut connection = pool.get().await.unwrap_or_else(|err| {
                log::error!("Failed to get database connection: {err}");
                std::process::exit(1);
            });
            let transaction = connection.transaction().await.unwrap_or_else(|err| {
                log::error!("Failed to start transaction: {err}");
                std::process::exit(1);
            });

            if let Ok(uuid) = uuid::Uuid::parse_str(&uuid) {
                let res = crate::cornucopia::queries::user::delete_by_uuid()
                    .bind(&transaction, &uuid)
                    .all()
                    .await;
                match res {
                    Ok(entries) if !entries.is_empty() => {
                        println!("User with UUID {uuid} removed successfully");

                        transaction.commit().await.unwrap_or_else(|err| {
                            log::error!("Failed to commit transaction: {err}");
                            std::process::exit(1);
                        });
                        return Ok(());
                    }
                    Ok(_) => {
                        log::error!("No user found with UUID: {uuid}");
                        transaction.rollback().await.unwrap_or_else(|err| {
                            log::error!("Failed to rollback transaction: {err}");
                            std::process::exit(1);
                        });
                        std::process::exit(1);
                    }
                    Err(err) => {
                        log::error!("Failed to remove user by UUID: {err}");
                        transaction.rollback().await.unwrap_or_else(|err| {
                            log::error!("Failed to rollback transaction: {err}");
                            std::process::exit(1);
                        });
                        std::process::exit(1);
                    }
                }
            } else {
                log::error!("Invalid UUID format: {uuid}");
                transaction.rollback().await.unwrap_or_else(|err| {
                    log::error!("Failed to rollback transaction: {err}");
                    std::process::exit(1);
                });
                std::process::exit(1);
            }
        }
        Command::User {
            user_command: UserCommand::Invite { name },
        } => {
            println!("Inviting user: {name}");
            let mut connection = pool.get().await.unwrap_or_else(|err| {
                log::error!("Failed to get database connection: {err}");
                std::process::exit(1);
            });
            let transaction = connection.transaction().await.unwrap_or_else(|err| {
                log::error!("Failed to start transaction: {err}");
                std::process::exit(1);
            });
            let user = match crate::cornucopia::queries::user::get_by_username()
                .bind(&transaction, &name)
                .one()
                .await
            {
                Ok(user) => user,
                Err(err) => {
                    log::error!("Failed to fetch user by name: {err}");
                    transaction.rollback().await.unwrap_or_else(|err| {
                        log::error!("Failed to rollback transaction: {err}");
                        std::process::exit(1);
                    });
                    std::process::exit(1);
                }
            };
            let invite_code = InviteQuery::new(user.uuid);
            let invite_code_str = invite_code.code;
            transaction.commit().await.unwrap_or_else(|err| {
                log::error!("Failed to commit transaction: {err}");
                std::process::exit(1);
            });
            println!(
                "Invite for user {name}: {}/api/invite?code={invite_code_str}",
                env!("PITSU_PUBLIC_URL")
            );
        }
        Command::Repo {
            repository_command: RepositoryCommand::List,
        } => {
            println!("Repositories:");
            let connection = pool.get().await.unwrap_or_else(|err| {
                log::error!("Failed to get database connection: {err}");
                std::process::exit(1);
            });
            let repos = crate::cornucopia::queries::repository::get_all()
                .bind(&connection)
                .all()
                .await
                .unwrap_or_else(|err| {
                    log::error!("Failed to list repositories: {err}");
                    std::process::exit(1);
                });
            for repo in repos.iter() {
                println!("- {} <{}>", repo.name, repo.uuid);
            }
            if repos.is_empty() {
                println!("No repositories found.");
            }
        }
        Command::Repo {
            repository_command: RepositoryCommand::Sync { stage },
        } => {
            if stage.sync_hashes() {
                println!("Syncing repository hashes...");
                let mut connection = pool.get().await.unwrap_or_else(|err| {
                    log::error!("Failed to get database connection: {err}");
                    std::process::exit(1);
                });

                let repos = crate::cornucopia::queries::repository::get_all()
                    .bind(&connection)
                    .all()
                    .await
                    .unwrap_or_else(|err| {
                        log::error!("Failed to fetch repositories: {err}");
                        std::process::exit(1);
                    });
                if repos.is_empty() {
                    println!("No repositories found to sync.");
                    return Ok(());
                }
                let total = repos.len();
                for (i, repo) in repos.iter().enumerate() {
                    let transaction = connection.transaction().await.unwrap_or_else(|err| {
                        log::error!("Failed to start transaction: {err}");
                        std::process::exit(1);
                    });
                    println!("Syncing repository {}/{total}: {}", i + 1, repo.name);

                    let root_path = std::env::var("ROOT_FOLDER").unwrap_or_else(|_| "repositories".to_string());
                    let full_path = format!("{}/{}", root_path, repo.uuid);

                    if !std::path::Path::new(&full_path).exists() {
                        log::warn!("Repository path does not exist: {full_path}");
                        continue;
                    }

                    let root_folder = match pitsu_lib::RootFolder::ingest_folder(&full_path.into()) {
                        Ok(folder) => folder,
                        Err(err) => {
                            log::error!("Failed to ingest folder for repository {}: {err}", repo.name);
                            continue;
                        }
                    };

                    let file_hashes = match serde_json::to_value(&root_folder) {
                        Ok(value) => value,
                        Err(err) => {
                            log::error!("Failed to serialize file hashes for repository {}: {err}", repo.name);
                            continue;
                        }
                    };

                    let res = crate::cornucopia::queries::repository::update_file_hashes_by_uuid()
                        .bind(&transaction, &file_hashes, &repo.uuid)
                        .one()
                        .await;
                    match res {
                        Ok(_) => {
                            println!("Successfully synced repository: {}", repo.name);

                            if let Err(err) = transaction.commit().await {
                                log::error!("Failed to commit transaction: {err}");
                            }
                        }
                        Err(err) => {
                            log::error!("Failed to update file hashes for repository {}: {err}", repo.name);

                            if let Err(rollback_err) = transaction.rollback().await {
                                log::error!("Failed to rollback transaction: {rollback_err}");
                            }
                        }
                    }
                }
            }
            if stage.sync_aws() {
                println!("Syncing AWS files...");
                match sync_aws_files(&pool, &s3_client, stage.aws_repo()).await {
                    Ok(_) => {
                        println!("Successfully synced AWS files");
                    }
                    Err(err) => {
                        log::error!("Failed to sync AWS files: {err}");
                    }
                }
            }
        }
    }
    Ok(())
}

async fn create_pool() -> Result<Pool, deadpool_postgres::CreatePoolError> {
    let mut cfg = deadpool_postgres::Config::new();
    cfg.user = Some(env!("POSTGRES_USER").to_string());
    cfg.password = Some(env!("POSTGRES_PASSWORD").to_string());
    cfg.host = Some(env!("POSTGRES_HOST").to_string());
    cfg.port = Some(env!("POSTGRES_PORT").parse().unwrap());
    cfg.dbname = Some(env!("POSTGRES_DB").to_string());
    cfg.create_pool(Some(deadpool_postgres::Runtime::Tokio1), postgres::NoTls)
}

pub async fn get_user(req: &actix_web::HttpRequest, pool: Arc<Pool>) -> Result<User, actix_web::Error> {
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let mut connection = pool
                    .get()
                    .await
                    .map_err(|_| actix_web::error::ErrorInternalServerError("Database connection error"))?;
                let transaction = connection
                    .transaction()
                    .await
                    .map_err(|_| actix_web::error::ErrorInternalServerError("Transaction error"))?;
                match cornucopia::queries::user::get_by_api_key()
                    .bind(&transaction, &token)
                    .one()
                    .await
                {
                    Ok(user) => {
                        return Ok(User {
                            uuid: user.uuid,
                            username: user.username.into(),
                        });
                    }
                    Err(_) => return Err(actix_web::error::ErrorUnauthorized("Invalid token")),
                }
            }
        }
    }
    Err(actix_web::error::ErrorUnauthorized(
        "Missing or invalid Authorization header",
    ))
}

pub async fn check_user_access(
    pool: Arc<Pool>,
    user_uuid: &uuid::Uuid,
    repo_uuid: &uuid::Uuid,
) -> Result<AccessLevel, actix_web::Error> {
    let mut connection = pool
        .get()
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Database connection error"))?;
    let transaction = connection
        .transaction()
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("Transaction error"))?;
    match cornucopia::queries::access::user_has_access()
        .bind(&transaction, user_uuid, repo_uuid)
        .one()
        .await
    {
        Ok(access) => Ok(Into::<AccessLevel>::into(access)),
        Err(_) => Err(actix_web::error::ErrorForbidden("Access denied")),
    }
}

impl From<cornucopia::types::public::AccessLevel> for AccessLevel {
    fn from(val: cornucopia::types::public::AccessLevel) -> Self {
        match val {
            cornucopia::types::public::AccessLevel::NONE => AccessLevel::None,
            cornucopia::types::public::AccessLevel::READ => AccessLevel::Read,
            cornucopia::types::public::AccessLevel::WRITE => AccessLevel::Write,
            cornucopia::types::public::AccessLevel::ADMIN => AccessLevel::Admin,
            cornucopia::types::public::AccessLevel::OWNER => AccessLevel::Owner,
        }
    }
}

impl From<AccessLevel> for cornucopia::types::public::AccessLevel {
    fn from(val: AccessLevel) -> Self {
        match val {
            AccessLevel::None => cornucopia::types::public::AccessLevel::NONE,
            AccessLevel::Read => cornucopia::types::public::AccessLevel::READ,
            AccessLevel::Write => cornucopia::types::public::AccessLevel::WRITE,
            AccessLevel::Admin => cornucopia::types::public::AccessLevel::ADMIN,
            AccessLevel::Owner => cornucopia::types::public::AccessLevel::OWNER,
        }
    }
}

async fn build_executable(api_key: Option<Arc<str>>) -> Result<PathBuf> {
    tokio::spawn(async {
        // Set the environment variables for the build
        // std::env::set_var("PITSU_API_KEY", api_key);
        // std::env::set_var("PITSU_API_USERNAME", api_username);
        // let version_number = {
        //     let output = std::process::Command::new("git")
        //         .arg("rev-parse")
        //         .arg("HEAD")
        //         .current_dir(env!("CARGO_MANIFEST_DIR"))
        //         .output()
        //         .map_err(|e| anyhow::anyhow!("Failed to get git commit hash: {}", e))?;
        //     if !output.status.success() {
        //         return Err(anyhow::anyhow!(
        //             "Git command failed: {}",
        //             String::from_utf8_lossy(&output.stderr)
        //         ));
        //     }
        //     String::from_utf8(output.stdout)
        //         .map_err(|e| anyhow::anyhow!("Failed to parse git commit hash: {}", e))?
        //         .trim()
        //         .to_string()
        // };
        let version_number = get_client_version()?;
        // std::env::set_var("VERSION_NUMBER", serde_json::to_string(&version_number)?);
        std::env::set_var("VERSION_MAJOR", version_number.major.to_string());
        std::env::set_var("VERSION_MINOR", version_number.minor.to_string());
        std::env::set_var("VERSION_PATCH", version_number.patch.to_string());
        std::env::set_var("VERSION_HASH", version_number.folder_hash);
        // Move to {crate_root}/local
        let crate_root = format!(
            "{}/../",
            std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string())
        );
        let local_path = format!("{crate_root}/local");
        std::env::set_current_dir(local_path)
            .map_err(|e| anyhow::anyhow!("Failed to change directory to local: {}", e))?;
        // Run the build command
        let output = tokio::process::Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg("x86_64-pc-windows-gnu")
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to run cargo build: {}", e))?;
        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Cargo build failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let mut executable_path = format!("{crate_root}/target/x86_64-pc-windows-gnu/release/pitsu.exe");
        if let Some(api_key) = api_key {
            // Read in the bytes, and replace the sequence env!("PITSU_PPITSU_API_KEY_PLACEHOLDER") with the users API key
            if !std::path::Path::new(&executable_path).exists() {
                return Err(anyhow::anyhow!("Executable not found at {executable_path}"));
            }
            let mut executable_file = tokio::fs::File::open(&executable_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open executable file: {}", e))?;
            let mut executable_bytes = Vec::new();
            executable_file
                .read_to_end(&mut executable_bytes)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to read executable file: {}", e))?;
            let api_key_placeholder =
                "________________________________PITSU_API_KEY_PLACEHOLDER________________________________";
            // let api_key_bytes = format!("{api_key}{}", "_".repeat(api_key.len() - api_key_placeholder.len()));
            let api_key_bytes = format!("__________________________{api_key}___________________________");
            let placeholder_bytes = api_key_placeholder.as_bytes();
            let mut modified_bytes = Vec::new();
            let mut start = 0;
            while start < executable_bytes.len() {
                if executable_bytes[start..].starts_with(placeholder_bytes) {
                    modified_bytes.extend_from_slice(api_key_bytes.as_bytes());
                    start += placeholder_bytes.len();
                } else {
                    modified_bytes.push(executable_bytes[start]);
                    start += 1;
                }
            }
            // Write the modified bytes to a new file in /tmp/pitsu/pitsu.{api_key}.exe
            let binaries_path = "/tmp/pitsu/".to_string();
            tokio::fs::create_dir_all(&binaries_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create binaries directory: {}", e))?;
            // let og_executable_path = executable_path.clone();
            // executable_path = format!("{binaries_path}/pitsu.{api_key}.exe");
            executable_path = format!("{binaries_path}/pitsu.exe");
            tokio::fs::remove_file(&executable_path).await.ok(); // Ignore error if file doesn't exist
                                                                 // tokio::fs::copy(&og_executable_path, &executable_path).await?;
            let mut modified_file = tokio::fs::File::create(&executable_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create modified executable file: {}", e))?;
            modified_file
                .write_all(&modified_bytes)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write modified executable file: {}", e))?;
            log::info!("Successfully created modified executable file at {executable_path}");
        }
        Ok(PathBuf::from(executable_path))
    })
    .await?
}

fn get_client_version() -> Result<VersionNumber> {
    let output = match std::process::Command::new("git")
        .arg("pull")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            log::error!("Failed to run git pull: {err}");
            return Err(anyhow::anyhow!("Failed to update local version"));
        }
    };
    if !output.status.success() {
        log::error!("Git pull failed: {}", String::from_utf8_lossy(&output.stderr));
        return Err(anyhow::anyhow!("Failed to update local version"));
    }
    // We need to read the line from the Cargo.toml file in ../local that has the version number
    let client_path = PathBuf::from(format!(
        "{}/../local",
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string())
    ));
    VersionNumber::new(&client_path)
}

async fn sync_aws_files(pool: &Pool, s3_client: &aws_sdk_s3::Client, only_this_repo: Option<Uuid>) -> Result<()> {
    let mut connection = pool
        .get()
        .await
        .map_err(|err| anyhow::anyhow!("Failed to get database connection: {err}"))?;
    let transaction = connection
        .transaction()
        .await
        .map_err(|err| anyhow::anyhow!("Failed to start transaction: {err}"))?;
    // we need to go through every single file in every single repository and ensure that they exist within the Files table (and have a valid aws S3 object key)
    let mut table_files: Vec<(Uuid, String, String)> = vec![];
    {
        let raw_table_files = cornucopia::queries::files::get_all()
            .bind(&transaction)
            .all()
            .await
            .map_err(|err| anyhow::anyhow!("Failed to fetch files: {err}"))?;
        for file in raw_table_files {
            // ensure the s3 object key is valid, if not, delete this entry from the database
            if s3_client
                .head_object()
                .bucket(env!("AWS_BUCKET_NAME"))
                .key(&file.aws_s3_object_key)
                .send()
                .await
                .is_err()
            {
                cornucopia::queries::files::delete()
                    .bind(&transaction, &file.file_path, &file.repository_uuid)
                    .one()
                    .await
                    .map_err(|err| anyhow::anyhow!("Failed to delete file with invalid S3 key: {err}"))?;
                continue;
            }
            table_files.push((file.repository_uuid, file.file_path, file.aws_s3_object_key));
        }
    }
    println!("Found {} files in the database", table_files.len());

    let mut repository_files: Vec<(Uuid, String)> = vec![];
    {
        // get all repository directories out of the root folder by iterating over every folder in ROOT_FOLDER and attempting to parse the folders name as a uuid
        let mut raw_repository_folders = vec![]; // will be walked later
        let mut read_dir = tokio::fs::read_dir(env!("ROOT_FOLDER"))
            .await
            .map_err(|err| anyhow::anyhow!("Failed to read root folder: {err}"))?;
        while let Ok(Some(file)) = read_dir.next_entry().await {
            if let Ok(uuid) = Uuid::parse_str(file.file_name().to_str().unwrap_or("")) {
                raw_repository_folders.push((uuid, file.path()));
            }
        }
        // walk the dirs and retrieve only the part of the path AFTER ROOT_FOLDER/uuid
        for (uuid, path) in raw_repository_folders {
            let mut walker = async_walkdir::WalkDir::new(&path);
            while let Some(Ok(file)) = walker.next().await {
                match file.metadata().await.map(|m| m.is_file()) {
                    Ok(true) => {
                        let relative_path = file
                            .path()
                            .strip_prefix(&path)
                            .expect("Failed to strip prefix")
                            .to_path_buf();
                        repository_files.push((uuid, relative_path.display().to_string()));
                    }
                    Ok(false) => {}
                    Err(e) => {
                        println!("Failed to get metadata for file {}: {}", file.path().display(), e);
                    }
                }
            }
        }
    }
    println!("Found {} repository files", repository_files.len());

    if let Some(only_this_repo) = only_this_repo {
        table_files.retain(|(uuid, _, _)| *uuid == only_this_repo);
        repository_files.retain(|(uuid, _)| *uuid == only_this_repo);
        println!(
            "Filtered to only repository {}, {} database files and {} repository files",
            only_this_repo,
            table_files.len(),
            repository_files.len()
        );
    }

    let table_files: Arc<[(Uuid, String, String)]> = Arc::from(table_files);

    let total = table_files.len();
    if total != 0 {
        display_percentage(total, 0, "Checking repository files");
    }
    // first walk every database entry and ensure the file exists on disk, if not, remove the entry from the database, and delete the file from the s3 bucket
    for (i, (uuid, file_path, s3_key)) in table_files.iter().enumerate() {
        if !repository_files.contains(&(*uuid, file_path.clone())) {
            cornucopia::queries::files::delete()
                .bind(&transaction, &file_path, uuid)
                .one()
                .await
                .map_err(|err| anyhow::anyhow!("Failed to delete file from database: {err}"))?;
            s3_client
                .delete_object()
                .bucket(env!("AWS_BUCKET_NAME"))
                .key(s3_key)
                .send()
                .await
                .map_err(|err| anyhow::anyhow!("Failed to delete file from S3: {err}"))?;
        }
        display_percentage(total, i + 1, "Checking repository files");
    }
    println!("Finished cleaning up database entries");

    transaction
        .commit()
        .await
        .map_err(|err| anyhow::anyhow!("Failed to commit transaction: {err}"))?;
    let total = repository_files.len();
    if total != 0 {
        display_percentage(total, 0, "Uploading new repository files");
    }
    // now walk every file on disk, if the file does not exist within the database, upload the file to S3, get the object ID, and create the database entry
    let mut orderless = FuturesUnordered::new();
    for (uuid, file_path) in repository_files.iter() {
        let pool = pool.clone();
        let table_files = table_files.clone();
        let client = s3_client.clone();
        orderless.push(tokio::spawn(download_and_insert(
            pool,
            table_files,
            client,
            *uuid,
            file_path.clone(),
        )))
    }
    let mut i = 0;
    while let Some(res) = orderless.next().await {
        i += 1;
        match res {
            Ok(Ok(false)) => {
                display_percentage(total, i, "Skipped existing repository file");
            }
            Ok(Ok(true)) => {
                display_percentage(total, i, "Uploaded new repository file");
            }
            Ok(Err(err)) => {
                log::error!("Failed to upload and insert file: {err}");
            }
            Err(err) => {
                log::error!("Task panicked: {err}");
            }
        }
        // display_percentage(total, total - orderless.len(), "Uploading new repository files");
    }
    println!("Finished uploading new repository files");

    // now iterate over every file in the bucket, and if their key does not exist in the database, delete them from s3
    let mut s3_keys: Vec<String> = vec![];
    let mut continuation_token = None;
    println!("Collecting AWS Keys");
    println!("Collected: 0");
    loop {
        let list_objects = if let Some(continuation_token) = continuation_token {
            s3_client
                .list_objects_v2()
                .bucket(env!("AWS_BUCKET_NAME"))
                .continuation_token(continuation_token)
                .send()
                .await?
        } else {
            s3_client
                .list_objects_v2()
                .bucket(env!("AWS_BUCKET_NAME"))
                .send()
                .await?
        };
        for object in list_objects.contents.as_ref().unwrap_or(&vec![]) {
            s3_keys.push(
                object
                    .key()
                    .map(|key| key.to_string())
                    .unwrap_or(String::from("INVALID_KEY")),
            );
        }
        if !list_objects.is_truncated.unwrap_or(false) {
            break;
        }
        continuation_token = list_objects.next_continuation_token().map(|s| s.to_string());
        println!("\rCollected: {}", s3_keys.len());
    }
    println!("Finished collecting AWS keys. Total: {}", s3_keys.len());

    // refresh table_files
    let transaction = connection
        .transaction()
        .await
        .map_err(|err| anyhow::anyhow!("Failed to start transaction: {err}"))?;
    let mut table_files: Vec<(Uuid, String, String)> = vec![];
    {
        let raw_table_files = cornucopia::queries::files::get_all()
            .bind(&transaction)
            .all()
            .await
            .map_err(|err| anyhow::anyhow!("Failed to fetch files: {err}"))?;
        for file in raw_table_files {
            // ensure the s3 object key is valid, if not, delete this entry from the database
            if s3_client
                .head_object()
                .bucket(env!("AWS_BUCKET_NAME"))
                .key(&file.aws_s3_object_key)
                .send()
                .await
                .is_err()
            {
                cornucopia::queries::files::delete()
                    .bind(&transaction, &file.file_path, &file.repository_uuid)
                    .one()
                    .await
                    .map_err(|err| anyhow::anyhow!("Failed to delete file with invalid S3 key: {err}"))?;
                continue;
            }
            table_files.push((file.repository_uuid, file.file_path, file.aws_s3_object_key));
        }
    }
    println!("Found {} files in the database", table_files.len());

    if let Some(only_this_repo) = only_this_repo {
        table_files.retain(|(uuid, _, _)| *uuid == only_this_repo);
        println!(
            "Filtered to only repository {}, {} database files",
            only_this_repo,
            table_files.len()
        );
    }

    if let Some(only_this_repo) = only_this_repo {
        s3_keys.retain(|key| {
            table_files
                .iter()
                .any(|(uuid, _, k)| *uuid == only_this_repo && k == key)
        });
        println!(
            "Filtered to only repository {}, {} S3 keys",
            only_this_repo,
            s3_keys.len()
        );
    }

    let delete_keys = s3_keys
        .iter()
        .filter(|s3_key| !table_files.iter().any(|(_, _, key)| key == *s3_key))
        .cloned()
        .collect::<Vec<String>>();

    let total = delete_keys.len();
    if total != 0 {
        display_percentage(total, 0, "Deleting orphaned S3 files");
    }
    for (i, s3_key) in delete_keys.iter().enumerate() {
        s3_client
            .delete_object()
            .bucket(env!("AWS_BUCKET_NAME"))
            .key(s3_key)
            .send()
            .await
            .map_err(|err| anyhow::anyhow!("Failed to delete file from S3: {err}"))?;
        display_percentage(total, i + 1, "Deleting orphaned S3 files");
    }
    println!("Finished deleting orphaned S3 files");

    Ok(())
}

fn display_percentage(total: usize, progress: usize, label: &str) {
    // Calculate the percentage
    let percentage = (progress as f64 / total as f64) * 100.0;
    // Print the percentage with the label, overwriting the previous line
    println!("{label}: {percentage:.2}% ({progress}/{total})");
}

async fn download_and_insert(
    pool: Pool,
    table_files: Arc<[(Uuid, String, String)]>,
    s3_client: S3Client,
    uuid: Uuid,
    file_path: String,
) -> Result<bool> {
    let mut connection = pool
        .get()
        .await
        .map_err(|err| anyhow::anyhow!("Failed to get database connection: {err}"))?;
    let transaction = connection
        .transaction()
        .await
        .map_err(|err| anyhow::anyhow!("Failed to start transaction: {err}"))?;
    if !table_files.iter().any(|(u, f, _)| u == &uuid && f == &file_path) {
        let s3_key = Uuid::new_v4().to_string();
        s3_client
            .put_object()
            .bucket(env!("AWS_BUCKET_NAME"))
            .key(&s3_key)
            .acl(aws_sdk_s3::types::ObjectCannedAcl::PublicRead)
            .body(
                aws_sdk_s3::primitives::ByteStream::from_path(format!(
                    "{}/{}/{}",
                    env!("ROOT_FOLDER"),
                    uuid,
                    file_path
                ))
                .await?,
            )
            .send()
            .await
            .map_err(|err| anyhow::anyhow!("Failed to upload file to S3: {}", DisplayErrorContext(err)))?;
        cornucopia::queries::files::create()
            .bind(&transaction, &uuid, &file_path, &s3_key)
            .one()
            .await
            .map_err(|err| anyhow::anyhow!("Failed to create file in database: {err}"))?;
        transaction
            .commit()
            .await
            .map_err(|err| anyhow::anyhow!("Failed to commit transaction: {err}"))?;
        Ok(true)
    } else {
        Ok(false)
    }
}
