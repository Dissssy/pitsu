// This file was generated with `cornucopia`. Do not modify.

#[allow(clippy::all, clippy::pedantic)] #[allow(unused_variables)]
#[allow(unused_imports)] #[allow(dead_code)] pub mod types { pub mod public { #[derive( Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)] pub enum AccessLevel { NONE,READ,WRITE,ADMIN,OWNER,}impl<'a> postgres_types::ToSql for AccessLevel
{
    fn
    to_sql(&self, ty: &postgres_types::Type, buf: &mut
    postgres_types::private::BytesMut,) -> Result<postgres_types::IsNull,
    Box<dyn std::error::Error + Sync + Send>,>
    {
        let s = match *self { AccessLevel::NONE => "NONE",AccessLevel::READ => "READ",AccessLevel::WRITE => "WRITE",AccessLevel::ADMIN => "ADMIN",AccessLevel::OWNER => "OWNER",};
        buf.extend_from_slice(s.as_bytes());
        std::result::Result::Ok(postgres_types::IsNull::No)
    } fn accepts(ty: &postgres_types::Type) -> bool
    {
        if ty.name() != "access_level" { return false; } match *ty.kind()
        {
            postgres_types::Kind::Enum(ref variants) =>
            {
                if variants.len() != 5 { return false; }
                variants.iter().all(|v| match &**v
                { "NONE" => true,"READ" => true,"WRITE" => true,"ADMIN" => true,"OWNER" => true,_ => false, })
            } _ => false,
        }
    } fn
    to_sql_checked(&self, ty: &postgres_types::Type, out: &mut
    postgres_types::private::BytesMut,) -> Result<postgres_types::IsNull,
    Box<dyn std::error::Error + Sync + Send>>
    { postgres_types::__to_sql_checked(self, ty, out) }
} impl<'a> postgres_types::FromSql<'a> for AccessLevel
{
    fn from_sql(ty: &postgres_types::Type, buf: &'a [u8],) ->
    Result<AccessLevel, Box<dyn std::error::Error + Sync + Send>,>
    {
        match std::str::from_utf8(buf)?
        {
            "NONE" => Ok(AccessLevel::NONE),"READ" => Ok(AccessLevel::READ),"WRITE" => Ok(AccessLevel::WRITE),"ADMIN" => Ok(AccessLevel::ADMIN),"OWNER" => Ok(AccessLevel::OWNER),s =>
            Result::Err(Into::into(format!("invalid variant `{}`", s))),
        }
    } fn accepts(ty: &postgres_types::Type) -> bool
    {
        if ty.name() != "access_level" { return false; } match *ty.kind()
        {
            postgres_types::Kind::Enum(ref variants) =>
            {
                if variants.len() != 5 { return false; }
                variants.iter().all(|v| match &**v
                { "NONE" => true,"READ" => true,"WRITE" => true,"ADMIN" => true,"OWNER" => true,_ => false, })
            } _ => false,
        }
    }
} }}#[allow(clippy::all, clippy::pedantic)] #[allow(unused_variables)]
#[allow(unused_imports)] #[allow(dead_code)] pub mod queries
{ pub mod access
{ use futures::{{StreamExt, TryStreamExt}};use futures; use cornucopia_async::GenericClient;#[derive(Clone,Copy, Debug)] pub struct CreateOrUpdateParams<> { pub repository_uuid: uuid::Uuid,pub user_uuid: uuid::Uuid,pub access_level: super::super::types::public::AccessLevel,}#[derive(Clone,Copy, Debug)] pub struct DeleteByUserUuidAndRepositoryUuidParams<> { pub user_uuid: uuid::Uuid,pub repository_uuid: uuid::Uuid,}#[derive(Clone,Copy, Debug)] pub struct UserHasAccessParams<> { pub user_uuid: uuid::Uuid,pub repository_uuid: uuid::Uuid,}#[derive( Debug, Clone, PartialEq,Copy)] pub struct DeleteByUserUuidAndRepositoryUuid
{ pub uuid : uuid::Uuid,pub repository_uuid : uuid::Uuid,pub user_uuid : uuid::Uuid,pub access_level : super::super::types::public::AccessLevel,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct DeleteByUserUuidAndRepositoryUuidQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> DeleteByUserUuidAndRepositoryUuid,
    mapper: fn(DeleteByUserUuidAndRepositoryUuid) -> T,
} impl<'a, C, T:'a, const N: usize> DeleteByUserUuidAndRepositoryUuidQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(DeleteByUserUuidAndRepositoryUuid) -> R) ->
    DeleteByUserUuidAndRepositoryUuidQuery<'a,C,R,N>
    {
        DeleteByUserUuidAndRepositoryUuidQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}pub struct SuperSuperTypesPublicAccessLevelQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> super::super::types::public::AccessLevel,
    mapper: fn(super::super::types::public::AccessLevel) -> T,
} impl<'a, C, T:'a, const N: usize> SuperSuperTypesPublicAccessLevelQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(super::super::types::public::AccessLevel) -> R) ->
    SuperSuperTypesPublicAccessLevelQuery<'a,C,R,N>
    {
        SuperSuperTypesPublicAccessLevelQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,Copy)] pub struct GetByUser
{ pub uuid : uuid::Uuid,pub repository_uuid : uuid::Uuid,pub user_uuid : uuid::Uuid,pub access_level : super::super::types::public::AccessLevel,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct GetByUserQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetByUser,
    mapper: fn(GetByUser) -> T,
} impl<'a, C, T:'a, const N: usize> GetByUserQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetByUser) -> R) ->
    GetByUserQuery<'a,C,R,N>
    {
        GetByUserQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct GetAllUsersWithAccess
{ pub user_uuid : uuid::Uuid,pub access_level : super::super::types::public::AccessLevel,pub username : String,}pub struct GetAllUsersWithAccessBorrowed<'a> { pub user_uuid : uuid::Uuid,pub access_level : super::super::types::public::AccessLevel,pub username : &'a str,}
impl<'a> From<GetAllUsersWithAccessBorrowed<'a>> for GetAllUsersWithAccess
{
    fn from(GetAllUsersWithAccessBorrowed { user_uuid,access_level,username,}: GetAllUsersWithAccessBorrowed<'a>) -> Self
    { Self { user_uuid,access_level,username: username.into(),} }
}pub struct GetAllUsersWithAccessQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetAllUsersWithAccessBorrowed,
    mapper: fn(GetAllUsersWithAccessBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> GetAllUsersWithAccessQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetAllUsersWithAccessBorrowed) -> R) ->
    GetAllUsersWithAccessQuery<'a,C,R,N>
    {
        GetAllUsersWithAccessQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}pub fn create_or_update() -> CreateOrUpdateStmt
{ CreateOrUpdateStmt(cornucopia_async::private::Stmt::new("INSERT INTO Access (repository_uuid, user_uuid, access_level)
    VALUES ($1, $2, $3)
    ON CONFLICT (repository_uuid, user_uuid)
    DO UPDATE SET access_level = EXCLUDED.access_level,
              updated_at = CURRENT_TIMESTAMP")) } pub struct
CreateOrUpdateStmt(cornucopia_async::private::Stmt); impl CreateOrUpdateStmt
{ pub async fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
repository_uuid: &'a uuid::Uuid,user_uuid: &'a uuid::Uuid,access_level: &'a super::super::types::public::AccessLevel,) -> Result<u64, tokio_postgres::Error>
{
    let stmt = self.0.prepare(client).await?;
    client.execute(stmt, &[repository_uuid,user_uuid,access_level,]).await
} }impl <'a, C: GenericClient + Send + Sync, >
cornucopia_async::Params<'a, CreateOrUpdateParams<>, std::pin::Pin<Box<dyn futures::Future<Output = Result<u64,
tokio_postgres::Error>> + Send + 'a>>, C> for CreateOrUpdateStmt
{
    fn
    params(&'a mut self, client: &'a  C, params: &'a
    CreateOrUpdateParams<>) -> std::pin::Pin<Box<dyn futures::Future<Output = Result<u64,
    tokio_postgres::Error>> + Send + 'a>>
    { Box::pin(self.bind(client, &params.repository_uuid,&params.user_uuid,&params.access_level,)) }
}pub fn delete_by_user_uuid_and_repository_uuid() -> DeleteByUserUuidAndRepositoryUuidStmt
{ DeleteByUserUuidAndRepositoryUuidStmt(cornucopia_async::private::Stmt::new("DELETE FROM Access
    WHERE user_uuid = $1 AND repository_uuid = $2
    RETURNING *")) } pub struct
DeleteByUserUuidAndRepositoryUuidStmt(cornucopia_async::private::Stmt); impl DeleteByUserUuidAndRepositoryUuidStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
user_uuid: &'a uuid::Uuid,repository_uuid: &'a uuid::Uuid,) -> DeleteByUserUuidAndRepositoryUuidQuery<'a,C, DeleteByUserUuidAndRepositoryUuid,
2>
{
    DeleteByUserUuidAndRepositoryUuidQuery
    {
        client, params: [user_uuid,repository_uuid,], stmt: &mut self.0, extractor:
        |row| { DeleteByUserUuidAndRepositoryUuid { uuid: row.get(0),repository_uuid: row.get(1),user_uuid: row.get(2),access_level: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <DeleteByUserUuidAndRepositoryUuid>::from(it) },
    }
} }impl <'a, C: GenericClient,> cornucopia_async::Params<'a,
DeleteByUserUuidAndRepositoryUuidParams<>, DeleteByUserUuidAndRepositoryUuidQuery<'a, C, DeleteByUserUuidAndRepositoryUuid,
2>, C> for DeleteByUserUuidAndRepositoryUuidStmt
{
    fn
    params(&'a mut self, client: &'a  C, params: &'a
    DeleteByUserUuidAndRepositoryUuidParams<>) -> DeleteByUserUuidAndRepositoryUuidQuery<'a, C,
    DeleteByUserUuidAndRepositoryUuid, 2>
    { self.bind(client, &params.user_uuid,&params.repository_uuid,) }
}pub fn user_has_access() -> UserHasAccessStmt
{ UserHasAccessStmt(cornucopia_async::private::Stmt::new("SELECT
    CASE
        WHEN r.owner_uuid = $1 THEN 'OWNER'
        ELSE COALESCE(a.access_level, 'NONE')
    END AS access_level
FROM Repositories r
LEFT JOIN Access a ON r.uuid = a.repository_uuid AND a.user_uuid = $1
WHERE r.uuid = $2")) } pub struct
UserHasAccessStmt(cornucopia_async::private::Stmt); impl UserHasAccessStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
user_uuid: &'a uuid::Uuid,repository_uuid: &'a uuid::Uuid,) -> SuperSuperTypesPublicAccessLevelQuery<'a,C, super::super::types::public::AccessLevel,
2>
{
    SuperSuperTypesPublicAccessLevelQuery
    {
        client, params: [user_uuid,repository_uuid,], stmt: &mut self.0, extractor:
        |row| { row.get(0) }, mapper: |it| { it },
    }
} }impl <'a, C: GenericClient,> cornucopia_async::Params<'a,
UserHasAccessParams<>, SuperSuperTypesPublicAccessLevelQuery<'a, C, super::super::types::public::AccessLevel,
2>, C> for UserHasAccessStmt
{
    fn
    params(&'a mut self, client: &'a  C, params: &'a
    UserHasAccessParams<>) -> SuperSuperTypesPublicAccessLevelQuery<'a, C,
    super::super::types::public::AccessLevel, 2>
    { self.bind(client, &params.user_uuid,&params.repository_uuid,) }
}pub fn get_by_user() -> GetByUserStmt
{ GetByUserStmt(cornucopia_async::private::Stmt::new("SELECT * FROM Access WHERE user_uuid = $1")) } pub struct
GetByUserStmt(cornucopia_async::private::Stmt); impl GetByUserStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
user_uuid: &'a uuid::Uuid,) -> GetByUserQuery<'a,C, GetByUser,
1>
{
    GetByUserQuery
    {
        client, params: [user_uuid,], stmt: &mut self.0, extractor:
        |row| { GetByUser { uuid: row.get(0),repository_uuid: row.get(1),user_uuid: row.get(2),access_level: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <GetByUser>::from(it) },
    }
} }pub fn get_all_users_with_access() -> GetAllUsersWithAccessStmt
{ GetAllUsersWithAccessStmt(cornucopia_async::private::Stmt::new("SELECT * FROM (
    SELECT
        r.owner_uuid AS user_uuid,
        'OWNER' AS access_level,
        u.username
    FROM Repositories r
    JOIN Users u ON r.owner_uuid = u.uuid
    WHERE r.uuid = $1

    UNION ALL

    SELECT
        a.user_uuid,
        a.access_level,
        u.username
    FROM Access a
    JOIN Users u ON a.user_uuid = u.uuid
    WHERE a.repository_uuid = $1
) AS access_info")) } pub struct
GetAllUsersWithAccessStmt(cornucopia_async::private::Stmt); impl GetAllUsersWithAccessStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
repository_uuid: &'a uuid::Uuid,) -> GetAllUsersWithAccessQuery<'a,C, GetAllUsersWithAccess,
1>
{
    GetAllUsersWithAccessQuery
    {
        client, params: [repository_uuid,], stmt: &mut self.0, extractor:
        |row| { GetAllUsersWithAccessBorrowed { user_uuid: row.get(0),access_level: row.get(1),username: row.get(2),} }, mapper: |it| { <GetAllUsersWithAccess>::from(it) },
    }
} }}pub mod repository
{ use futures::{{StreamExt, TryStreamExt}};use futures; use cornucopia_async::GenericClient;#[derive( Debug)] pub struct CreateParams<T1: cornucopia_async::StringSql,> { pub name: T1,pub owner_uuid: uuid::Uuid,}#[derive( Debug)] pub struct GetByNameAndOwnerParams<T1: cornucopia_async::StringSql,> { pub name: T1,pub owner_uuid: uuid::Uuid,}#[derive( Debug)] pub struct UpdateFileHashesByUuidParams<T1: cornucopia_async::JsonSql,> { pub file_hashes: T1,pub uuid: uuid::Uuid,}#[derive( Debug)] pub struct UpdateMetadataByUuidParams<T1: cornucopia_async::StringSql,> { pub name: T1,pub uuid: uuid::Uuid,}#[derive( Debug, Clone, PartialEq,)] pub struct Create
{ pub uuid : uuid::Uuid,pub name : String,pub owner_uuid : uuid::Uuid,pub file_hashes : serde_json::Value,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct CreateBorrowed<'a> { pub uuid : uuid::Uuid,pub name : &'a str,pub owner_uuid : uuid::Uuid,pub file_hashes : postgres_types::Json<&'a serde_json::value::RawValue>,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<CreateBorrowed<'a>> for Create
{
    fn from(CreateBorrowed { uuid,name,owner_uuid,file_hashes,created_at,updated_at,}: CreateBorrowed<'a>) -> Self
    { Self { uuid,name: name.into(),owner_uuid,file_hashes: serde_json::from_str(file_hashes.0.get()).unwrap(),created_at,updated_at,} }
}pub struct CreateQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> CreateBorrowed,
    mapper: fn(CreateBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> CreateQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(CreateBorrowed) -> R) ->
    CreateQuery<'a,C,R,N>
    {
        CreateQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct DeleteByUuid
{ pub uuid : uuid::Uuid,pub name : String,pub owner_uuid : uuid::Uuid,pub file_hashes : serde_json::Value,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct DeleteByUuidBorrowed<'a> { pub uuid : uuid::Uuid,pub name : &'a str,pub owner_uuid : uuid::Uuid,pub file_hashes : postgres_types::Json<&'a serde_json::value::RawValue>,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<DeleteByUuidBorrowed<'a>> for DeleteByUuid
{
    fn from(DeleteByUuidBorrowed { uuid,name,owner_uuid,file_hashes,created_at,updated_at,}: DeleteByUuidBorrowed<'a>) -> Self
    { Self { uuid,name: name.into(),owner_uuid,file_hashes: serde_json::from_str(file_hashes.0.get()).unwrap(),created_at,updated_at,} }
}pub struct DeleteByUuidQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> DeleteByUuidBorrowed,
    mapper: fn(DeleteByUuidBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> DeleteByUuidQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(DeleteByUuidBorrowed) -> R) ->
    DeleteByUuidQuery<'a,C,R,N>
    {
        DeleteByUuidQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct GetByUuid
{ pub uuid : uuid::Uuid,pub name : String,pub owner_uuid : uuid::Uuid,pub file_hashes : serde_json::Value,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct GetByUuidBorrowed<'a> { pub uuid : uuid::Uuid,pub name : &'a str,pub owner_uuid : uuid::Uuid,pub file_hashes : postgres_types::Json<&'a serde_json::value::RawValue>,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<GetByUuidBorrowed<'a>> for GetByUuid
{
    fn from(GetByUuidBorrowed { uuid,name,owner_uuid,file_hashes,created_at,updated_at,}: GetByUuidBorrowed<'a>) -> Self
    { Self { uuid,name: name.into(),owner_uuid,file_hashes: serde_json::from_str(file_hashes.0.get()).unwrap(),created_at,updated_at,} }
}pub struct GetByUuidQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetByUuidBorrowed,
    mapper: fn(GetByUuidBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> GetByUuidQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetByUuidBorrowed) -> R) ->
    GetByUuidQuery<'a,C,R,N>
    {
        GetByUuidQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct GetByNameAndOwner
{ pub uuid : uuid::Uuid,pub name : String,pub owner_uuid : uuid::Uuid,pub file_hashes : serde_json::Value,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct GetByNameAndOwnerBorrowed<'a> { pub uuid : uuid::Uuid,pub name : &'a str,pub owner_uuid : uuid::Uuid,pub file_hashes : postgres_types::Json<&'a serde_json::value::RawValue>,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<GetByNameAndOwnerBorrowed<'a>> for GetByNameAndOwner
{
    fn from(GetByNameAndOwnerBorrowed { uuid,name,owner_uuid,file_hashes,created_at,updated_at,}: GetByNameAndOwnerBorrowed<'a>) -> Self
    { Self { uuid,name: name.into(),owner_uuid,file_hashes: serde_json::from_str(file_hashes.0.get()).unwrap(),created_at,updated_at,} }
}pub struct GetByNameAndOwnerQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetByNameAndOwnerBorrowed,
    mapper: fn(GetByNameAndOwnerBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> GetByNameAndOwnerQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetByNameAndOwnerBorrowed) -> R) ->
    GetByNameAndOwnerQuery<'a,C,R,N>
    {
        GetByNameAndOwnerQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct GetByOwner
{ pub uuid : uuid::Uuid,pub name : String,pub owner_uuid : uuid::Uuid,pub file_hashes : serde_json::Value,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct GetByOwnerBorrowed<'a> { pub uuid : uuid::Uuid,pub name : &'a str,pub owner_uuid : uuid::Uuid,pub file_hashes : postgres_types::Json<&'a serde_json::value::RawValue>,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<GetByOwnerBorrowed<'a>> for GetByOwner
{
    fn from(GetByOwnerBorrowed { uuid,name,owner_uuid,file_hashes,created_at,updated_at,}: GetByOwnerBorrowed<'a>) -> Self
    { Self { uuid,name: name.into(),owner_uuid,file_hashes: serde_json::from_str(file_hashes.0.get()).unwrap(),created_at,updated_at,} }
}pub struct GetByOwnerQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetByOwnerBorrowed,
    mapper: fn(GetByOwnerBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> GetByOwnerQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetByOwnerBorrowed) -> R) ->
    GetByOwnerQuery<'a,C,R,N>
    {
        GetByOwnerQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct GetAll
{ pub uuid : uuid::Uuid,pub name : String,pub owner_uuid : uuid::Uuid,pub file_hashes : serde_json::Value,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct GetAllBorrowed<'a> { pub uuid : uuid::Uuid,pub name : &'a str,pub owner_uuid : uuid::Uuid,pub file_hashes : postgres_types::Json<&'a serde_json::value::RawValue>,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<GetAllBorrowed<'a>> for GetAll
{
    fn from(GetAllBorrowed { uuid,name,owner_uuid,file_hashes,created_at,updated_at,}: GetAllBorrowed<'a>) -> Self
    { Self { uuid,name: name.into(),owner_uuid,file_hashes: serde_json::from_str(file_hashes.0.get()).unwrap(),created_at,updated_at,} }
}pub struct GetAllQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetAllBorrowed,
    mapper: fn(GetAllBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> GetAllQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetAllBorrowed) -> R) ->
    GetAllQuery<'a,C,R,N>
    {
        GetAllQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct UpdateFileHashesByUuid
{ pub uuid : uuid::Uuid,pub name : String,pub owner_uuid : uuid::Uuid,pub file_hashes : serde_json::Value,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct UpdateFileHashesByUuidBorrowed<'a> { pub uuid : uuid::Uuid,pub name : &'a str,pub owner_uuid : uuid::Uuid,pub file_hashes : postgres_types::Json<&'a serde_json::value::RawValue>,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<UpdateFileHashesByUuidBorrowed<'a>> for UpdateFileHashesByUuid
{
    fn from(UpdateFileHashesByUuidBorrowed { uuid,name,owner_uuid,file_hashes,created_at,updated_at,}: UpdateFileHashesByUuidBorrowed<'a>) -> Self
    { Self { uuid,name: name.into(),owner_uuid,file_hashes: serde_json::from_str(file_hashes.0.get()).unwrap(),created_at,updated_at,} }
}pub struct UpdateFileHashesByUuidQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> UpdateFileHashesByUuidBorrowed,
    mapper: fn(UpdateFileHashesByUuidBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> UpdateFileHashesByUuidQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(UpdateFileHashesByUuidBorrowed) -> R) ->
    UpdateFileHashesByUuidQuery<'a,C,R,N>
    {
        UpdateFileHashesByUuidQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct UpdateMetadataByUuid
{ pub uuid : uuid::Uuid,pub name : String,pub owner_uuid : uuid::Uuid,pub file_hashes : serde_json::Value,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct UpdateMetadataByUuidBorrowed<'a> { pub uuid : uuid::Uuid,pub name : &'a str,pub owner_uuid : uuid::Uuid,pub file_hashes : postgres_types::Json<&'a serde_json::value::RawValue>,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<UpdateMetadataByUuidBorrowed<'a>> for UpdateMetadataByUuid
{
    fn from(UpdateMetadataByUuidBorrowed { uuid,name,owner_uuid,file_hashes,created_at,updated_at,}: UpdateMetadataByUuidBorrowed<'a>) -> Self
    { Self { uuid,name: name.into(),owner_uuid,file_hashes: serde_json::from_str(file_hashes.0.get()).unwrap(),created_at,updated_at,} }
}pub struct UpdateMetadataByUuidQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> UpdateMetadataByUuidBorrowed,
    mapper: fn(UpdateMetadataByUuidBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> UpdateMetadataByUuidQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(UpdateMetadataByUuidBorrowed) -> R) ->
    UpdateMetadataByUuidQuery<'a,C,R,N>
    {
        UpdateMetadataByUuidQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}pub fn create() -> CreateStmt
{ CreateStmt(cornucopia_async::private::Stmt::new("INSERT INTO Repositories (name, owner_uuid)
    VALUES ($1, $2)
    RETURNING *")) } pub struct
CreateStmt(cornucopia_async::private::Stmt); impl CreateStmt
{ pub fn bind<'a, C:
GenericClient,T1:
cornucopia_async::StringSql,>(&'a mut self, client: &'a  C,
name: &'a T1,owner_uuid: &'a uuid::Uuid,) -> CreateQuery<'a,C, Create,
2>
{
    CreateQuery
    {
        client, params: [name,owner_uuid,], stmt: &mut self.0, extractor:
        |row| { CreateBorrowed { uuid: row.get(0),name: row.get(1),owner_uuid: row.get(2),file_hashes: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <Create>::from(it) },
    }
} }impl <'a, C: GenericClient,T1: cornucopia_async::StringSql,> cornucopia_async::Params<'a,
CreateParams<T1,>, CreateQuery<'a, C, Create,
2>, C> for CreateStmt
{
    fn
    params(&'a mut self, client: &'a  C, params: &'a
    CreateParams<T1,>) -> CreateQuery<'a, C,
    Create, 2>
    { self.bind(client, &params.name,&params.owner_uuid,) }
}pub fn delete_by_uuid() -> DeleteByUuidStmt
{ DeleteByUuidStmt(cornucopia_async::private::Stmt::new("DELETE FROM Repositories
    WHERE uuid = $1
    RETURNING *")) } pub struct
DeleteByUuidStmt(cornucopia_async::private::Stmt); impl DeleteByUuidStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
uuid: &'a uuid::Uuid,) -> DeleteByUuidQuery<'a,C, DeleteByUuid,
1>
{
    DeleteByUuidQuery
    {
        client, params: [uuid,], stmt: &mut self.0, extractor:
        |row| { DeleteByUuidBorrowed { uuid: row.get(0),name: row.get(1),owner_uuid: row.get(2),file_hashes: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <DeleteByUuid>::from(it) },
    }
} }pub fn get_by_uuid() -> GetByUuidStmt
{ GetByUuidStmt(cornucopia_async::private::Stmt::new("SELECT * FROM Repositories
    WHERE uuid = $1")) } pub struct
GetByUuidStmt(cornucopia_async::private::Stmt); impl GetByUuidStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
uuid: &'a uuid::Uuid,) -> GetByUuidQuery<'a,C, GetByUuid,
1>
{
    GetByUuidQuery
    {
        client, params: [uuid,], stmt: &mut self.0, extractor:
        |row| { GetByUuidBorrowed { uuid: row.get(0),name: row.get(1),owner_uuid: row.get(2),file_hashes: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <GetByUuid>::from(it) },
    }
} }pub fn get_by_name_and_owner() -> GetByNameAndOwnerStmt
{ GetByNameAndOwnerStmt(cornucopia_async::private::Stmt::new("SELECT * FROM Repositories
    WHERE name = $1 AND owner_uuid = $2")) } pub struct
GetByNameAndOwnerStmt(cornucopia_async::private::Stmt); impl GetByNameAndOwnerStmt
{ pub fn bind<'a, C:
GenericClient,T1:
cornucopia_async::StringSql,>(&'a mut self, client: &'a  C,
name: &'a T1,owner_uuid: &'a uuid::Uuid,) -> GetByNameAndOwnerQuery<'a,C, GetByNameAndOwner,
2>
{
    GetByNameAndOwnerQuery
    {
        client, params: [name,owner_uuid,], stmt: &mut self.0, extractor:
        |row| { GetByNameAndOwnerBorrowed { uuid: row.get(0),name: row.get(1),owner_uuid: row.get(2),file_hashes: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <GetByNameAndOwner>::from(it) },
    }
} }impl <'a, C: GenericClient,T1: cornucopia_async::StringSql,> cornucopia_async::Params<'a,
GetByNameAndOwnerParams<T1,>, GetByNameAndOwnerQuery<'a, C, GetByNameAndOwner,
2>, C> for GetByNameAndOwnerStmt
{
    fn
    params(&'a mut self, client: &'a  C, params: &'a
    GetByNameAndOwnerParams<T1,>) -> GetByNameAndOwnerQuery<'a, C,
    GetByNameAndOwner, 2>
    { self.bind(client, &params.name,&params.owner_uuid,) }
}pub fn get_by_owner() -> GetByOwnerStmt
{ GetByOwnerStmt(cornucopia_async::private::Stmt::new("SELECT * FROM Repositories
    WHERE owner_uuid = $1
    ORDER BY created_at DESC")) } pub struct
GetByOwnerStmt(cornucopia_async::private::Stmt); impl GetByOwnerStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
owner_uuid: &'a uuid::Uuid,) -> GetByOwnerQuery<'a,C, GetByOwner,
1>
{
    GetByOwnerQuery
    {
        client, params: [owner_uuid,], stmt: &mut self.0, extractor:
        |row| { GetByOwnerBorrowed { uuid: row.get(0),name: row.get(1),owner_uuid: row.get(2),file_hashes: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <GetByOwner>::from(it) },
    }
} }pub fn get_all() -> GetAllStmt
{ GetAllStmt(cornucopia_async::private::Stmt::new("SELECT * FROM Repositories
    ORDER BY created_at DESC")) } pub struct
GetAllStmt(cornucopia_async::private::Stmt); impl GetAllStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
) -> GetAllQuery<'a,C, GetAll,
0>
{
    GetAllQuery
    {
        client, params: [], stmt: &mut self.0, extractor:
        |row| { GetAllBorrowed { uuid: row.get(0),name: row.get(1),owner_uuid: row.get(2),file_hashes: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <GetAll>::from(it) },
    }
} }pub fn update_file_hashes_by_uuid() -> UpdateFileHashesByUuidStmt
{ UpdateFileHashesByUuidStmt(cornucopia_async::private::Stmt::new("UPDATE Repositories
    SET file_hashes = $1
    WHERE uuid = $2
    RETURNING *")) } pub struct
UpdateFileHashesByUuidStmt(cornucopia_async::private::Stmt); impl UpdateFileHashesByUuidStmt
{ pub fn bind<'a, C:
GenericClient,T1:
cornucopia_async::JsonSql,>(&'a mut self, client: &'a  C,
file_hashes: &'a T1,uuid: &'a uuid::Uuid,) -> UpdateFileHashesByUuidQuery<'a,C, UpdateFileHashesByUuid,
2>
{
    UpdateFileHashesByUuidQuery
    {
        client, params: [file_hashes,uuid,], stmt: &mut self.0, extractor:
        |row| { UpdateFileHashesByUuidBorrowed { uuid: row.get(0),name: row.get(1),owner_uuid: row.get(2),file_hashes: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <UpdateFileHashesByUuid>::from(it) },
    }
} }impl <'a, C: GenericClient,T1: cornucopia_async::JsonSql,> cornucopia_async::Params<'a,
UpdateFileHashesByUuidParams<T1,>, UpdateFileHashesByUuidQuery<'a, C, UpdateFileHashesByUuid,
2>, C> for UpdateFileHashesByUuidStmt
{
    fn
    params(&'a mut self, client: &'a  C, params: &'a
    UpdateFileHashesByUuidParams<T1,>) -> UpdateFileHashesByUuidQuery<'a, C,
    UpdateFileHashesByUuid, 2>
    { self.bind(client, &params.file_hashes,&params.uuid,) }
}pub fn update_metadata_by_uuid() -> UpdateMetadataByUuidStmt
{ UpdateMetadataByUuidStmt(cornucopia_async::private::Stmt::new("UPDATE Repositories
    SET name = $1, updated_at = CURRENT_TIMESTAMP
    WHERE uuid = $2
    RETURNING *")) } pub struct
UpdateMetadataByUuidStmt(cornucopia_async::private::Stmt); impl UpdateMetadataByUuidStmt
{ pub fn bind<'a, C:
GenericClient,T1:
cornucopia_async::StringSql,>(&'a mut self, client: &'a  C,
name: &'a T1,uuid: &'a uuid::Uuid,) -> UpdateMetadataByUuidQuery<'a,C, UpdateMetadataByUuid,
2>
{
    UpdateMetadataByUuidQuery
    {
        client, params: [name,uuid,], stmt: &mut self.0, extractor:
        |row| { UpdateMetadataByUuidBorrowed { uuid: row.get(0),name: row.get(1),owner_uuid: row.get(2),file_hashes: row.get(3),created_at: row.get(4),updated_at: row.get(5),} }, mapper: |it| { <UpdateMetadataByUuid>::from(it) },
    }
} }impl <'a, C: GenericClient,T1: cornucopia_async::StringSql,> cornucopia_async::Params<'a,
UpdateMetadataByUuidParams<T1,>, UpdateMetadataByUuidQuery<'a, C, UpdateMetadataByUuid,
2>, C> for UpdateMetadataByUuidStmt
{
    fn
    params(&'a mut self, client: &'a  C, params: &'a
    UpdateMetadataByUuidParams<T1,>) -> UpdateMetadataByUuidQuery<'a, C,
    UpdateMetadataByUuid, 2>
    { self.bind(client, &params.name,&params.uuid,) }
}}pub mod user
{ use futures::{{StreamExt, TryStreamExt}};use futures; use cornucopia_async::GenericClient;#[derive( Debug, Clone, PartialEq,)] pub struct Create
{ pub uuid : uuid::Uuid,pub username : String,pub api_key : String,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct CreateBorrowed<'a> { pub uuid : uuid::Uuid,pub username : &'a str,pub api_key : &'a str,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<CreateBorrowed<'a>> for Create
{
    fn from(CreateBorrowed { uuid,username,api_key,created_at,updated_at,}: CreateBorrowed<'a>) -> Self
    { Self { uuid,username: username.into(),api_key: api_key.into(),created_at,updated_at,} }
}pub struct CreateQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> CreateBorrowed,
    mapper: fn(CreateBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> CreateQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(CreateBorrowed) -> R) ->
    CreateQuery<'a,C,R,N>
    {
        CreateQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct DeleteByUuid
{ pub uuid : uuid::Uuid,pub username : String,pub api_key : String,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct DeleteByUuidBorrowed<'a> { pub uuid : uuid::Uuid,pub username : &'a str,pub api_key : &'a str,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<DeleteByUuidBorrowed<'a>> for DeleteByUuid
{
    fn from(DeleteByUuidBorrowed { uuid,username,api_key,created_at,updated_at,}: DeleteByUuidBorrowed<'a>) -> Self
    { Self { uuid,username: username.into(),api_key: api_key.into(),created_at,updated_at,} }
}pub struct DeleteByUuidQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> DeleteByUuidBorrowed,
    mapper: fn(DeleteByUuidBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> DeleteByUuidQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(DeleteByUuidBorrowed) -> R) ->
    DeleteByUuidQuery<'a,C,R,N>
    {
        DeleteByUuidQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct GetByUuid
{ pub uuid : uuid::Uuid,pub username : String,pub api_key : String,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct GetByUuidBorrowed<'a> { pub uuid : uuid::Uuid,pub username : &'a str,pub api_key : &'a str,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<GetByUuidBorrowed<'a>> for GetByUuid
{
    fn from(GetByUuidBorrowed { uuid,username,api_key,created_at,updated_at,}: GetByUuidBorrowed<'a>) -> Self
    { Self { uuid,username: username.into(),api_key: api_key.into(),created_at,updated_at,} }
}pub struct GetByUuidQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetByUuidBorrowed,
    mapper: fn(GetByUuidBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> GetByUuidQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetByUuidBorrowed) -> R) ->
    GetByUuidQuery<'a,C,R,N>
    {
        GetByUuidQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct GetByUsername
{ pub uuid : uuid::Uuid,pub username : String,pub api_key : String,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct GetByUsernameBorrowed<'a> { pub uuid : uuid::Uuid,pub username : &'a str,pub api_key : &'a str,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<GetByUsernameBorrowed<'a>> for GetByUsername
{
    fn from(GetByUsernameBorrowed { uuid,username,api_key,created_at,updated_at,}: GetByUsernameBorrowed<'a>) -> Self
    { Self { uuid,username: username.into(),api_key: api_key.into(),created_at,updated_at,} }
}pub struct GetByUsernameQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetByUsernameBorrowed,
    mapper: fn(GetByUsernameBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> GetByUsernameQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetByUsernameBorrowed) -> R) ->
    GetByUsernameQuery<'a,C,R,N>
    {
        GetByUsernameQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct GetByApiKey
{ pub uuid : uuid::Uuid,pub username : String,pub api_key : String,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct GetByApiKeyBorrowed<'a> { pub uuid : uuid::Uuid,pub username : &'a str,pub api_key : &'a str,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<GetByApiKeyBorrowed<'a>> for GetByApiKey
{
    fn from(GetByApiKeyBorrowed { uuid,username,api_key,created_at,updated_at,}: GetByApiKeyBorrowed<'a>) -> Self
    { Self { uuid,username: username.into(),api_key: api_key.into(),created_at,updated_at,} }
}pub struct GetByApiKeyQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetByApiKeyBorrowed,
    mapper: fn(GetByApiKeyBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> GetByApiKeyQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetByApiKeyBorrowed) -> R) ->
    GetByApiKeyQuery<'a,C,R,N>
    {
        GetByApiKeyQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}#[derive( Debug, Clone, PartialEq,)] pub struct GetAll
{ pub uuid : uuid::Uuid,pub username : String,pub api_key : String,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}pub struct GetAllBorrowed<'a> { pub uuid : uuid::Uuid,pub username : &'a str,pub api_key : &'a str,pub created_at : time::PrimitiveDateTime,pub updated_at : time::PrimitiveDateTime,}
impl<'a> From<GetAllBorrowed<'a>> for GetAll
{
    fn from(GetAllBorrowed { uuid,username,api_key,created_at,updated_at,}: GetAllBorrowed<'a>) -> Self
    { Self { uuid,username: username.into(),api_key: api_key.into(),created_at,updated_at,} }
}pub struct GetAllQuery<'a, C: GenericClient, T, const N: usize>
{
    client: &'a  C, params:
    [&'a (dyn postgres_types::ToSql + Sync); N], stmt: &'a mut
    cornucopia_async::private::Stmt, extractor: fn(&tokio_postgres::Row) -> GetAllBorrowed,
    mapper: fn(GetAllBorrowed) -> T,
} impl<'a, C, T:'a, const N: usize> GetAllQuery<'a, C, T, N> where C:
GenericClient
{
    pub fn map<R>(self, mapper: fn(GetAllBorrowed) -> R) ->
    GetAllQuery<'a,C,R,N>
    {
        GetAllQuery
        {
            client: self.client, params: self.params, stmt: self.stmt,
            extractor: self.extractor, mapper,
        }
    } pub async fn one(self) -> Result<T, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let row =
        self.client.query_one(stmt, &self.params).await?;
        Ok((self.mapper)((self.extractor)(&row)))
    } pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error>
    { self.iter().await?.try_collect().await } pub async fn opt(self) ->
    Result<Option<T>, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?;
        Ok(self.client.query_opt(stmt, &self.params) .await?
        .map(|row| (self.mapper)((self.extractor)(&row))))
    } pub async fn iter(self,) -> Result<impl futures::Stream<Item = Result<T,
    tokio_postgres::Error>> + 'a, tokio_postgres::Error>
    {
        let stmt = self.stmt.prepare(self.client).await?; let it =
        self.client.query_raw(stmt,
        cornucopia_async::private::slice_iter(&self.params)) .await?
        .map(move |res|
        res.map(|row| (self.mapper)((self.extractor)(&row)))) .into_stream();
        Ok(it)
    }
}pub fn create() -> CreateStmt
{ CreateStmt(cornucopia_async::private::Stmt::new("INSERT INTO Users (username)
    VALUES ($1)
    RETURNING *")) } pub struct
CreateStmt(cornucopia_async::private::Stmt); impl CreateStmt
{ pub fn bind<'a, C:
GenericClient,T1:
cornucopia_async::StringSql,>(&'a mut self, client: &'a  C,
username: &'a T1,) -> CreateQuery<'a,C, Create,
1>
{
    CreateQuery
    {
        client, params: [username,], stmt: &mut self.0, extractor:
        |row| { CreateBorrowed { uuid: row.get(0),username: row.get(1),api_key: row.get(2),created_at: row.get(3),updated_at: row.get(4),} }, mapper: |it| { <Create>::from(it) },
    }
} }pub fn delete_by_uuid() -> DeleteByUuidStmt
{ DeleteByUuidStmt(cornucopia_async::private::Stmt::new("DELETE FROM Users
    WHERE uuid = $1
    RETURNING *")) } pub struct
DeleteByUuidStmt(cornucopia_async::private::Stmt); impl DeleteByUuidStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
uuid: &'a uuid::Uuid,) -> DeleteByUuidQuery<'a,C, DeleteByUuid,
1>
{
    DeleteByUuidQuery
    {
        client, params: [uuid,], stmt: &mut self.0, extractor:
        |row| { DeleteByUuidBorrowed { uuid: row.get(0),username: row.get(1),api_key: row.get(2),created_at: row.get(3),updated_at: row.get(4),} }, mapper: |it| { <DeleteByUuid>::from(it) },
    }
} }pub fn get_by_uuid() -> GetByUuidStmt
{ GetByUuidStmt(cornucopia_async::private::Stmt::new("SELECT * FROM Users
    WHERE uuid = $1")) } pub struct
GetByUuidStmt(cornucopia_async::private::Stmt); impl GetByUuidStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
uuid: &'a uuid::Uuid,) -> GetByUuidQuery<'a,C, GetByUuid,
1>
{
    GetByUuidQuery
    {
        client, params: [uuid,], stmt: &mut self.0, extractor:
        |row| { GetByUuidBorrowed { uuid: row.get(0),username: row.get(1),api_key: row.get(2),created_at: row.get(3),updated_at: row.get(4),} }, mapper: |it| { <GetByUuid>::from(it) },
    }
} }pub fn get_by_username() -> GetByUsernameStmt
{ GetByUsernameStmt(cornucopia_async::private::Stmt::new("SELECT * FROM Users
    WHERE username = $1")) } pub struct
GetByUsernameStmt(cornucopia_async::private::Stmt); impl GetByUsernameStmt
{ pub fn bind<'a, C:
GenericClient,T1:
cornucopia_async::StringSql,>(&'a mut self, client: &'a  C,
username: &'a T1,) -> GetByUsernameQuery<'a,C, GetByUsername,
1>
{
    GetByUsernameQuery
    {
        client, params: [username,], stmt: &mut self.0, extractor:
        |row| { GetByUsernameBorrowed { uuid: row.get(0),username: row.get(1),api_key: row.get(2),created_at: row.get(3),updated_at: row.get(4),} }, mapper: |it| { <GetByUsername>::from(it) },
    }
} }pub fn get_by_api_key() -> GetByApiKeyStmt
{ GetByApiKeyStmt(cornucopia_async::private::Stmt::new("SELECT * FROM Users
    WHERE api_key = $1")) } pub struct
GetByApiKeyStmt(cornucopia_async::private::Stmt); impl GetByApiKeyStmt
{ pub fn bind<'a, C:
GenericClient,T1:
cornucopia_async::StringSql,>(&'a mut self, client: &'a  C,
api_key: &'a T1,) -> GetByApiKeyQuery<'a,C, GetByApiKey,
1>
{
    GetByApiKeyQuery
    {
        client, params: [api_key,], stmt: &mut self.0, extractor:
        |row| { GetByApiKeyBorrowed { uuid: row.get(0),username: row.get(1),api_key: row.get(2),created_at: row.get(3),updated_at: row.get(4),} }, mapper: |it| { <GetByApiKey>::from(it) },
    }
} }pub fn get_all() -> GetAllStmt
{ GetAllStmt(cornucopia_async::private::Stmt::new("SELECT * FROM Users
    ORDER BY created_at DESC")) } pub struct
GetAllStmt(cornucopia_async::private::Stmt); impl GetAllStmt
{ pub fn bind<'a, C:
GenericClient,>(&'a mut self, client: &'a  C,
) -> GetAllQuery<'a,C, GetAll,
0>
{
    GetAllQuery
    {
        client, params: [], stmt: &mut self.0, extractor:
        |row| { GetAllBorrowed { uuid: row.get(0),username: row.get(1),api_key: row.get(2),created_at: row.get(3),updated_at: row.get(4),} }, mapper: |it| { <GetAll>::from(it) },
    }
} }}}