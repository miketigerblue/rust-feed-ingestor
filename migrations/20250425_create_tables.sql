CREATE TABLE IF NOT EXISTS archive (
    guid       TEXT   PRIMARY KEY,
    title      TEXT   NOT NULL,
    link       TEXT   NOT NULL,
    published  TIMESTAMP,
    content    TEXT
);

CREATE TABLE IF NOT EXISTS current (
    guid       TEXT   PRIMARY KEY,
    title      TEXT   NOT NULL,
    link       TEXT   NOT NULL,
    published  TIMESTAMP,
    content    TEXT
);
