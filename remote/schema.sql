CREATE TABLE Users (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT NOT NULL UNIQUE,
    api_key TEXT NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE Repositories (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    owner_uuid UUID NOT NULL REFERENCES Users(uuid) ON DELETE CASCADE,
    file_hashes JSONB NOT NULL DEFAULT '{}', -- Updated when someone with write access pushes changes
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TYPE access_level AS ENUM ('NONE', 'READ', 'WRITE', 'ADMIN', 'OWNER');

CREATE TABLE Access (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_uuid UUID NOT NULL REFERENCES Repositories(uuid) ON DELETE CASCADE,
    user_uuid UUID NOT NULL REFERENCES Users(uuid) ON DELETE CASCADE,
    -- access_level TEXT NOT NULL CHECK (access_level IN ('READ', 'WRITE', 'ADMIN')),
    access_level access_level NOT NULL CHECK (access_level IN ('READ', 'WRITE', 'ADMIN')), -- 'NONE' and 'OWNER' are not included here as they are generated on-the-fly during queries
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (repository_uuid, user_uuid)
);