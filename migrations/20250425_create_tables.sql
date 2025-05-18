-- ======================================================
-- Migration: Create or update archive and current tables
-- ======================================================

-- Archive table: stores raw feed items with metadata
CREATE TABLE IF NOT EXISTS archive (
    guid TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    link TEXT NOT NULL,
    published TIMESTAMP,
    content TEXT,
    full_content TEXT, -- Full content of the post
    summary TEXT,
    author TEXT,
    categories TEXT[], -- Postgres array of tags/categories
    entry_updated TIMESTAMP,
    -- Feed/source metadata
    feed_url TEXT NOT NULL, -- The canonical URL of the feed
    feed_title TEXT,
    feed_description TEXT,
    feed_language TEXT,
    feed_icon TEXT,
    feed_updated TIMESTAMP
);

-- Current table: mirrors archive but for current/latest items
CREATE TABLE IF NOT EXISTS current (
    guid TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    link TEXT NOT NULL,
    published TIMESTAMP,
    content TEXT,
    full_content TEXT, -- Full content of the post
    summary TEXT,
    author TEXT,
    categories TEXT[],
    entry_updated TIMESTAMP,
    -- Feed/source metadata
    feed_url TEXT NOT NULL,
    feed_title TEXT,
    feed_description TEXT,
    feed_language TEXT,
    feed_icon TEXT,
    feed_updated TIMESTAMP
);