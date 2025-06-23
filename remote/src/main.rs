use std::sync::Arc;

use actix_web::{
    delete, get, patch, post,
    web::{Data, Json, JsonConfig},
    App, HttpResponse, HttpServer, Responder,
};
use clap::Parser as _;
mod cornucopia;
use deadpool_postgres::Pool;
use pitsu_lib::{
    anyhow::Result, AccessLevel, CreateRemoteRepository, FileUpload, RemoteRepository, RootFolder,
    SimpleRemoteRepository, ThisUser, UpdateRemoteRepository, User, UserWithAccess,
};

// curl -X GET https://pit.p51.nl/
#[get("/")]
async fn root() -> impl Responder {
    HttpResponse::Ok().body(format!(
        "Planet51 Internet Transfer and Synchronization Utility Version {}",
        env!("CARGO_PKG_VERSION")
    ))
}

// curl -X GET https://pit.p51.nl/api
#[get("/api")]
async fn api() -> impl Responder {
    HttpResponse::Ok().body(format!(
        "Planet51 Internet Transfer and Synchronization Utility API Version {}",
        env!("CARGO_PKG_VERSION")
    ))
}

// curl -X GET https://pit.p51.nl/api/{path}
#[get("/api/{path:.*}")]
async fn api_catch_all(path: actix_web::web::Path<String>) -> impl Responder {
    let path = path.into_inner();
    log::debug!("API catch-all route hit with path: {path}");
    HttpResponse::NotFound().body(format!(
        "API endpoint not found: {path}. Please check the documentation for available endpoints."
    ))
}

// curl -X GET -H "Authorization Bearer <token>" https://pit.p51.nl/api/user
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
    let owned_repositories: Vec<SimpleRemoteRepository> =
        match cornucopia::queries::repository::get_by_owner()
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
                            return HttpResponse::InternalServerError()
                                .body("Failed to parse file hashes");
                        }
                    };
                    new_repos.push(SimpleRemoteRepository {
                        uuid: repo.uuid,
                        name: repo.name.into(),
                        size: files.size(),
                        file_count: files.file_count(),
                    });
                }
                new_repos
            }
            Err(err) => {
                log::error!("Failed to fetch owned repositories: {err}");
                return HttpResponse::InternalServerError()
                    .body("Failed to fetch owned repositories");
            }
        };
    let accessible_repositories: Vec<(SimpleRemoteRepository, AccessLevel)> =
        match cornucopia::queries::access::get_by_user()
            .bind(&transaction, &user.uuid)
            .all()
            .await
        {
            Ok(access) => {
                let mut new_accessible_repos: Vec<(SimpleRemoteRepository, AccessLevel)> =
                    Vec::with_capacity(access.len());
                for access_entry in access {
                    let repo = match cornucopia::queries::repository::get_by_uuid()
                        .bind(&transaction, &access_entry.repository_uuid)
                        .one()
                        .await
                    {
                        Ok(repo) => repo,
                        Err(err) => {
                            log::error!("Failed to fetch repository: {err}");
                            return HttpResponse::InternalServerError()
                                .body("Failed to fetch repository");
                        }
                    };
                    let files: RootFolder = match serde_json::from_value(repo.file_hashes) {
                        Ok(files) => files,
                        Err(err) => {
                            log::error!("Failed to parse file hashes: {err}");
                            return HttpResponse::InternalServerError()
                                .body("Failed to parse file hashes");
                        }
                    };
                    new_accessible_repos.push((
                        SimpleRemoteRepository {
                            uuid: repo.uuid,
                            name: repo.name.into(),
                            size: files.size(),
                            file_count: files.file_count(),
                        },
                        access_entry
                            .access_level
                            .try_into()
                            .unwrap_or(AccessLevel::None),
                    ));
                }
                new_accessible_repos
            }
            Err(err) => {
                log::error!("Failed to fetch accessible repositories: {err}");
                return HttpResponse::InternalServerError()
                    .body("Failed to fetch accessible repositories");
            }
        };
    HttpResponse::Ok().json(ThisUser {
        user,
        owned_repositories,
        accessible_repositories,
    })
}

// curl -X GET -H "Authorization Bearer <token>" https://pit.p51.nl/api/user/{uuid}
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
    // Fetch user information from the database
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

// curl -X GET -H "Authorization Bearer <token>" https://pit.p51.nl/api/user/
#[get("/api/user/")]
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
    // Fetch all users from the database
    match cornucopia::queries::user::get_all()
        .bind(&transaction)
        .all()
        .await
    {
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

// curl -X GET -H "Authorization Bearer <token>" https://pit.p51.nl/api/user/{uuid}
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

    // Check if the user has access to the repository
    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };

    if access_level == AccessLevel::None {
        log::warn!(
            "User {} does not have access to repository {}",
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
    // Fetch repository information from the database
    cornucopia::queries::repository::get_by_uuid()
        .bind(&transaction, &uuid)
        .one()
        .await
        .map(|repo| {
            let files = match serde_json::from_value(repo.file_hashes) {
                Ok(files) => files,
                Err(err) => {
                    log::error!("Failed to parse file hashes: {err}");
                    return HttpResponse::InternalServerError().body("Failed to parse file hashes");
                }
            };
            HttpResponse::Ok().json(RemoteRepository {
                uuid: repo.uuid,
                name: repo.name.into(),
                files,
                access_level,
            })
        })
        .unwrap_or_else(|err| {
            log::debug!("Failed to fetch repository: {err}");
            HttpResponse::InternalServerError().body("Failed to fetch repository")
        })
}

// curl -X PATCH -H "Content-Type: application/json" -d '{"name": "New Repository"}' -H "Authorization: Bearer <token>" https://pit.p51.nl/{uuid}
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

    // Check if the user has access to the repository
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

    // Update the repository metadata in the database
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
            transaction.rollback().await.ok(); // Ignore rollback errors
            HttpResponse::InternalServerError().body("Failed to update repository")
        }
    }
}

// curl -X GET -H "Authorization Bearer <token>" https://pit.p51.nl/api/access/{uuid}
#[get("/api/access/{uuid}")]
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

    // Check if the user has admin access to the repository
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

    // Fetch users with access to the repository
    match cornucopia::queries::access::get_all_users_with_access()
        .bind(&transaction, &uuid)
        .all()
        .await
    {
        Ok(access_list) => {
            let mut users_with_access: Vec<UserWithAccess> = Vec::with_capacity(access_list.len());
            for access in access_list {
                let Ok(access_level) = AccessLevel::try_from(access.access_level.as_str()) else {
                    log::error!(
                        "Invalid access level in database: {} for user {}",
                        access.access_level,
                        access.user_uuid
                    );
                    continue;
                };
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

// curl -X GET -H "Authorization: Bearer <token>" https://pit.p51.nl/api/repository/{uuid}/{path}
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

    // Check if the user has access to the repository
    let access_level = match check_user_access(pool.clone(), &user.uuid, &uuid).await {
        Ok(level) => level,
        Err(err) => {
            log::error!("Failed to check user access: {err}");
            return HttpResponse::Forbidden().body("Access denied");
        }
    };

    if access_level == AccessLevel::None {
        log::warn!(
            "User {} does not have access to repository {}",
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
    // Fetch repository information from the database
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
    // Check if the path exists
    if std::path::Path::new(&full_path).exists() {
        if std::path::Path::new(&full_path).is_dir() {
            // If it's a directory, return an index of the files within and all subfiles
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
                    // Return the index as a JSON response
                    return HttpResponse::Ok().json(index);
                }
                Err(err) => {
                    log::error!("Failed to index folder: {err}");
                    return HttpResponse::InternalServerError().body("Failed to index folder");
                }
            };
        }
        // If it exists, return the file
        match actix_files::NamedFile::open(full_path) {
            Ok(file) => file.into_response(&req),
            Err(err) => {
                log::error!("Failed to open file: {err}");
                HttpResponse::InternalServerError().body("File not found")
            }
        }
    } else {
        // If it doesn't exist, return a 404
        HttpResponse::NotFound().body("File not found")
    }
}

// curl -X POST -H "Authorization Bearer <token>" -F "file=@/path/to/file" https://pit.p51.nl/{uuid}/{path}
#[post("{uuid}/.pit/upload")]
async fn upload_file(
    req: actix_web::HttpRequest,
    uuid: actix_web::web::Path<uuid::Uuid>,
    pool: Data<Pool>,
    mut body: Json<FileUpload>,
) -> impl Responder {
    // BEHAVIOUR:
    // 1. Check if the user has write access to the repository
    // 2. Ensure all directories in the path exist
    // 3. Write the file to the specified path
    // - If the file exists, overwrite it
    // - If the user does not have write access, return 403 Forbidden
    // - If the repository does not exist, return 404 Not Found
    // - If the path is invalid (e.g., trying to write outside the repository), return 400 Bad Request
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    // let (uuid, path) = path_stuff.into_inner();
    // Check if the user has access to the repository
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
    // Fetch repository information from the database
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
        // Ensure the directory exists
        if let Some(parent) = std::path::Path::new(&full_path).parent() {
            if let Err(err) = tokio::fs::create_dir_all(parent).await {
                log::error!("Failed to create directory: {err}");
                return HttpResponse::InternalServerError().body("Failed to create directory");
            }
        }
        // retrieve file bytes
        let bytes = match file.get_bytes() {
            Ok(bytes) => bytes,
            Err(err) => {
                log::error!("Failed to get file bytes: {err}");
                return HttpResponse::BadRequest().body("Invalid file data");
            }
        };
        // Write the file to the specified path
        if let Err(err) = tokio::fs::write(&full_path, &bytes).await {
            log::error!("Failed to write file: {err}");
            return HttpResponse::InternalServerError().body("Failed to write file");
        }
        cleanup_paths.push(full_path.clone());
    }
    // Update the repository file hashes
    let root_folder = match RootFolder::ingest_folder(&repo_path.into()) {
        Ok(folder) => folder,
        Err(err) => {
            log::error!("Failed to ingest folder: {err}");
            // tokio::fs::remove_file(&full_path).await.ok(); // Clean up the file if ingestion fails
            for path in cleanup_paths {
                let _ = tokio::fs::remove_file(&path).await; // Clean up all files written
            }
            return HttpResponse::InternalServerError().body("Failed to ingest folder");
        }
    };
    let file_hashes = match serde_json::to_value(&root_folder) {
        Ok(value) => value,
        Err(err) => {
            log::error!("Failed to serialize file hashes: {err}");
            // tokio::fs::remove_file(&full_path).await.ok(); // Clean up the file if ingestion fails
            for path in cleanup_paths {
                let _ = tokio::fs::remove_file(&path).await; // Clean up all files written
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
                // tokio::fs::remove_file(&full_path).await.ok(); // Clean up the file if ingestion fails
                for path in cleanup_paths {
                    let _ = tokio::fs::remove_file(&path).await; // Clean up all files written
                }
                return HttpResponse::InternalServerError().body("Failed to commit changes");
            }
            HttpResponse::Ok().body("File uploaded successfully")
        }
        Err(err) => {
            log::error!("Failed to update file hashes: {err}");
            // tokio::fs::remove_file(&full_path).await.ok(); // Clean up the file if ingestion fails
            for path in cleanup_paths {
                let _ = tokio::fs::remove_file(&path).await; // Clean up all files written
            }
            transaction.rollback().await.ok(); // Ignore rollback errors
            HttpResponse::InternalServerError().body("Failed to update file hashes")
        }
    }
}

// #[derive(Debug, MultipartForm)]
// struct FileUpload {
//     #[multipart(limit = "100MB")]
//     file: Bytes,
// }

// curl -X DELETE -H "Authorization Bearer <token>" https://pit.p51.nl/{uuid}/{path}
#[delete("/{uuid}/{path:.*}")]
async fn delete_file(
    req: actix_web::HttpRequest,
    path_stuff: actix_web::web::Path<(uuid::Uuid, String)>,
    pool: Data<Pool>,
) -> impl Responder {
    // BEHAVIOUR:
    // 1. Check if the user has write access to the repository
    // 2. Delete the file or directory (and all its contents) at the specified path
    // - If the file does not exist, return 404 Not Found
    // - If the user does not have write access, return 403 Forbidden
    // - If the repository does not exist, return 404 Not Found
    let pool = pool.into_inner();
    let user = match get_user(&req, pool.clone()).await {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to get bearer token: {err}");
            return HttpResponse::Unauthorized().body("Unauthorized");
        }
    };
    let (uuid, path) = path_stuff.into_inner();
    // Check if the user has access to the repository
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
    // Fetch repository information from the database
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
    // Check if the path exists
    let metadata = match tokio::fs::metadata(&full_path).await {
        Ok(meta) => meta,
        Err(err) => {
            log::debug!("Failed to get metadata for path: {err}");
            return HttpResponse::NotFound().body("File or directory not found");
        }
    };

    // Delete the file or directory
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
    // Update the repository file hashes
    let root_folder =
        match RootFolder::ingest_folder(&format!("{}/{}", root_path, repo.uuid).into()) {
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
            transaction.rollback().await.ok(); // Ignore rollback errors
            HttpResponse::InternalServerError().body("Failed to update file hashes")
        }
    }
}

// curl -X POST -H "Content-Type: application/json" -d '{"name": "New Repository"}' -H "Authorization Bearer <token>" https://pit.p51.nl/api/repository
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

async fn exec(host: String, port: u16, pool: Pool) -> Result<()> {
    std::env::set_var("SEQ_API_KEY", env!("REMOTE_SEQ_API_KEY"));
    if let Err(e) = datalust_logger::init("pitsu") {
        eprintln!("Failed to initialize logger: {e}");
        std::process::exit(1);
    };
    let json_cfg = JsonConfig::default().limit(
        100 * 1024 * 1024, // 100 MB
    );
    HttpServer::new(move || {
        App::new()
            .app_data(json_cfg.clone())
            .app_data(Data::new(pool.clone()))
            .service(root)
            // Api Routes
            .service(api)
            .service(get_self)
            .service(get_other)
            .service(get_all_users)
            .service(get_users_with_access)
            .service(create_repository)
            // Wildcard routes (must be last to avoid conflicts)
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

// Cli:
// remote run --port 8080 --host 0.0.0.0
// remote user add --name alice
// remote user list
// remote user remove --name alice
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
            let res = crate::cornucopia::queries::user::create()
                .bind(&transaction, &name)
                .await;
            if let Err(err) = res {
                log::error!("Failed to add user: {err}");
                transaction.rollback().await.unwrap_or_else(|err| {
                    log::error!("Failed to rollback transaction: {err}");
                    std::process::exit(1);
                });
                std::process::exit(1);
            }
            transaction.commit().await.unwrap_or_else(|err| {
                log::error!("Failed to commit transaction: {err}");
                std::process::exit(1);
            });
            println!("User {name} added successfully");
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
            // Check if a uuid is being provided instead of a username
            if let Ok(uuid) = uuid::Uuid::parse_str(&uuid) {
                // If it's a UUID, we need to delete by UUID
                let res = crate::cornucopia::queries::user::delete_by_uuid()
                    .bind(&transaction, &uuid)
                    .all()
                    .await;
                match res {
                    Ok(entries) if !entries.is_empty() => {
                        // Successfully removed user
                        println!("User with UUID {uuid} removed successfully");
                        // Commit the transaction
                        transaction.commit().await.unwrap_or_else(|err| {
                            log::error!("Failed to commit transaction: {err}");
                            std::process::exit(1);
                        });
                        return Ok(());
                    }
                    Ok(_) => {
                        // No user found with that UUID
                        log::error!("No user found with UUID: {uuid}");
                        transaction.rollback().await.unwrap_or_else(|err| {
                            log::error!("Failed to rollback transaction: {err}");
                            std::process::exit(1);
                        });
                        std::process::exit(1);
                    }
                    Err(err) => {
                        // Error removing user
                        log::error!("Failed to remove user by UUID: {err}");
                        transaction.rollback().await.unwrap_or_else(|err| {
                            log::error!("Failed to rollback transaction: {err}");
                            std::process::exit(1);
                        });
                        std::process::exit(1);
                    }
                }
            } else {
                // Error
                log::error!("Invalid UUID format: {uuid}");
                transaction.rollback().await.unwrap_or_else(|err| {
                    log::error!("Failed to rollback transaction: {err}");
                    std::process::exit(1);
                });
                std::process::exit(1);
            }
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
            // Fetch all repositories
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
                // get path for root of repository
                let root_path =
                    std::env::var("ROOT_FOLDER").unwrap_or_else(|_| "repositories".to_string());
                let full_path = format!("{}/{}", root_path, repo.uuid);
                // Check if the path exists
                if !std::path::Path::new(&full_path).exists() {
                    log::warn!("Repository path does not exist: {full_path}");
                    continue;
                }
                // Ingest the folder structure
                let root_folder = match pitsu_lib::RootFolder::ingest_folder(&full_path.into()) {
                    Ok(folder) => folder,
                    Err(err) => {
                        log::error!(
                            "Failed to ingest folder for repository {}: {err}",
                            repo.name
                        );
                        continue;
                    }
                };
                // Store the file_hashes in the database
                let file_hashes = match serde_json::to_value(&root_folder) {
                    Ok(value) => value,
                    Err(err) => {
                        log::error!(
                            "Failed to serialize file hashes for repository {}: {err}",
                            repo.name
                        );
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
                        // Commit the transaction
                        if let Err(err) = transaction.commit().await {
                            log::error!("Failed to commit transaction: {err}");
                        }
                    }
                    Err(err) => {
                        log::error!(
                            "Failed to update file hashes for repository {}: {err}",
                            repo.name
                        );
                        // Rollback the transaction
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

pub async fn get_user(
    req: &actix_web::HttpRequest,
    pool: Arc<Pool>,
) -> Result<User, actix_web::Error> {
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                // Validate the token against the database
                let mut connection = pool.get().await.map_err(|_| {
                    actix_web::error::ErrorInternalServerError("Database connection error")
                })?;
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
    // Check if the user has access via ownership, or the
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
        Ok(access) => access.try_into().map_err(|_| {
            actix_web::error::ErrorInternalServerError("Failed to parse access level")
        }),
        Err(_) => Err(actix_web::error::ErrorForbidden("Access denied")),
    }
}
