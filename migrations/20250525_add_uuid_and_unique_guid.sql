-- Enable uuid-ossp extension if not already enabled
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ======== ARCHIVE TABLE ========

-- Drop old PK constraint if it exists (likely on guid)
ALTER TABLE archive DROP CONSTRAINT IF EXISTS archive_pkey;

-- Add id column if missing (but not yet PK)
ALTER TABLE archive
    ADD COLUMN IF NOT EXISTS id UUID DEFAULT uuid_generate_v4();

-- Backfill NULL ids (if table not empty and any are null)
UPDATE archive SET id = uuid_generate_v4() WHERE id IS NULL;

-- Set id as the new primary key
ALTER TABLE archive
    ADD PRIMARY KEY (id);

-- Add inserted_at column (if missing)
ALTER TABLE archive
    ADD COLUMN IF NOT EXISTS inserted_at TIMESTAMP NOT NULL DEFAULT NOW();

-- Ensure guid is unique (but NOT PK)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE tablename = 'archive' AND indexname = 'archive_guid_key'
    ) THEN
        CREATE UNIQUE INDEX archive_guid_key ON archive(guid);
    END IF;
END$$;

-- ======== CURRENT TABLE ========

-- Drop old PK constraint if it exists (likely on guid)
ALTER TABLE current DROP CONSTRAINT IF EXISTS current_pkey;

-- Add id column if missing
ALTER TABLE current
    ADD COLUMN IF NOT EXISTS id UUID DEFAULT uuid_generate_v4();

-- Backfill NULL ids
UPDATE current SET id = uuid_generate_v4() WHERE id IS NULL;

-- Set id as new primary key
ALTER TABLE current
    ADD PRIMARY KEY (id);

-- Add inserted_at column (if missing)
ALTER TABLE current
    ADD COLUMN IF NOT EXISTS inserted_at TIMESTAMP NOT NULL DEFAULT NOW();

-- Ensure guid is unique (but NOT PK)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE tablename = 'current' AND indexname = 'current_guid_key'
    ) THEN
        CREATE UNIQUE INDEX current_guid_key ON current(guid);
    END IF;
END$$;
