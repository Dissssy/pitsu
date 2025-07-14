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

CREATE TABLE Access (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_uuid UUID NOT NULL REFERENCES Repositories(uuid) ON DELETE CASCADE,
    user_uuid UUID NOT NULL REFERENCES Users(uuid) ON DELETE CASCADE,
    access_level TEXT NOT NULL CHECK (access_level IN ('READ', 'WRITE', 'ADMIN')),
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);