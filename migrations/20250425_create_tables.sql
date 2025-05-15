-- ARCHIVE TABLE
CREATE TABLE IF NOT EXISTS archive (
    guid TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    link TEXT NOT NULL,
    published TIMESTAMP,
    content TEXT,
    summary TEXT,
    author TEXT,
    categories TEXT[],           -- Postgres array of tags/categories
    entry_updated TIMESTAMP,

    -- Feed/source metadata:
    feed_url TEXT NOT NULL,      -- The canonical URL of the feed
    feed_title TEXT,
    feed_description TEXT,
    feed_language TEXT,
    feed_icon TEXT,
    feed_updated TIMESTAMP
);

-- CURRENT TABLE
CREATE TABLE IF NOT EXISTS current (
    guid TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    link TEXT NOT NULL,
    published TIMESTAMP,
    content TEXT,
    summary TEXT,
    author TEXT,
    categories TEXT[],
    entry_updated TIMESTAMP,

    -- Feed/source metadata:
    feed_url TEXT NOT NULL,
    feed_title TEXT,
    feed_description TEXT,
    feed_language TEXT,
    feed_icon TEXT,
    feed_updated TIMESTAMP
);