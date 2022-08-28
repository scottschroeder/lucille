-- Add migration script here
CREATE TABLE IF NOT EXISTS corpus
(
    id          INTEGER PRIMARY KEY NOT NULL,
    title       TEXT                NOT NULL
);

CREATE TABLE IF NOT EXISTS chapter
(
    id          INTEGER PRIMARY KEY NOT NULL,
    corpus_id   INTEGER             NOT NULL,
    title       TEXT                NOT NULL,
    season      INTEGER,
    episode     INTEGER,
    hash        BLOB                NOT NULL,
    FOREIGN KEY(corpus_id) REFERENCES corpus(id)
);

CREATE TABLE IF NOT EXISTS subtitle
(
    id          INTEGER PRIMARY KEY NOT NULL,
    chapter_id   INTEGER             NOT NULL,
    idx      INTEGER NOT NULL,
    content       TEXT                NOT NULL,
    time_start       TEXT                NOT NULL,
    time_end       TEXT                NOT NULL,
    FOREIGN KEY(chapter_id) REFERENCES chapter(id)
);
