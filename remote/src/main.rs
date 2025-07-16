use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
};

use actix_web::{
    delete, get, patch, post,
    web::{Data, Json, JsonConfig},
    App, HttpResponse, HttpServer, Responder,
};
use clap::Parser as _;
mod cornucopia;
use deadpool_postgres::Pool;
use pitsu_lib::{
    anyhow::{self, Result},
    decode_string_base64, encode_string_base64, AccessLevel, CreateRemoteRepository, FileUpload, RemoteRepository,
    RootFolder, SimpleRemoteRepository, ThisUser, UpdateRemoteRepository, User, UserWithAccess, VersionNumber,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::cornucopia::queries::access::get_all_users_with_access;

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
    let mut _connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match _connection.transaction().await {
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
    let mut _connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match _connection.transaction().await {
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
    let mut _connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match _connection.transaction().await {
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

    let mut _connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match _connection.transaction().await {
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
                    //
                    HttpResponse::Ok().json(RemoteRepository {
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

    let mut _connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match _connection.transaction().await {
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

    let mut _connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match _connection.transaction().await {
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

    let mut _connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match _connection.transaction().await {
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
                Ok(index) => {
                    return HttpResponse::Ok().json(index);
                }
                Err(err) => {
                    log::error!("Failed to index folder: {err}");
                    return HttpResponse::InternalServerError().body("Failed to index folder");
                }
            };
        }

        match actix_files::NamedFile::open(full_path) {
            Ok(file) => file.into_response(&req),
            Err(err) => {
                log::error!("Failed to open file: {err}");
                HttpResponse::InternalServerError().body("File not found")
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
    let mut _connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match _connection.transaction().await {
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
        let full_path = format!("{repo_path}/{}", file.path);
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
        cleanup_paths.push(full_path.clone());
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
    let mut _connection = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            log::error!("Failed to get database connection: {err}");
            return HttpResponse::InternalServerError().body("Database connection error");
        }
    };
    let transaction = match _connection.transaction().await {
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
        .await;
    if let Err(err) = res {
        log::error!("Failed to create repository: {err}");
        transaction.rollback().await.unwrap_or_else(|err| {
            log::error!("Failed to rollback transaction: {err}");
        });
        return HttpResponse::InternalServerError().body("Failed to create repository");
    }
    transaction.commit().await.unwrap_or_else(|err| {
        log::error!("Failed to commit transaction: {err}");
    });
    HttpResponse::Created().body("Repository created successfully")
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
        let _user = match cornucopia::queries::user::get_by_uuid()
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
        match build_executable().await {
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
    let path = match build_executable().await {
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

async fn exec(host: String, port: u16, pool: Pool) -> Result<()> {
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
            .service(root)
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
    Sync,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    let pool = create_pool().await.unwrap_or_else(|err| {
        log::error!("Failed to create database pool: {err}");
        std::process::exit(1);
    });

    match cli.command {
        Command::Run { port, host } => {
            exec(host, port, pool).await.unwrap_or_else(|err| {
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
            repository_command: RepositoryCommand::Sync,
        } => {
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

async fn build_executable() -> Result<PathBuf> {
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
        // Return the path to the built executable
        let executable_path = format!("{crate_root}/target/x86_64-pc-windows-gnu/release/pitsu.exe");
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
