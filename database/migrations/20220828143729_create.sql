-- Add migration script here
CREATE TABLE IF NOT EXISTS corpus
(
    id          INTEGER PRIMARY KEY NOT NULL,
    title       TEXT                NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS chapter
(
    id          INTEGER PRIMARY KEY NOT NULL,
    corpus_id   INTEGER             NOT NULL,
    title       TEXT                NOT NULL,
    season      INTEGER,
    episode     INTEGER,
    hash        TEXT                NOT NULL UNIQUE,
    FOREIGN KEY(corpus_id) REFERENCES corpus(id)
);

CREATE TABLE IF NOT EXISTS srtfile
(
    id          INTEGER PRIMARY KEY NOT NULL,
    uuid        TEXT NOT            NULL UNIQUE,
    chapter_id   INTEGER             NOT NULL,
    data    BLOB NOT NULL,
    FOREIGN KEY(chapter_id) REFERENCES chapter(id)
);

CREATE TABLE IF NOT EXISTS media_view
(
    id          INTEGER PRIMARY KEY NOT NULL,
    chapter_id   INTEGER             NOT NULL,
    name       TEXT                NOT NULL,

    FOREIGN KEY(chapter_id) REFERENCES chapter(id),
    UNIQUE(chapter_id, name)
);

CREATE TABLE IF NOT EXISTS media_segment
(
    id          INTEGER PRIMARY KEY NOT NULL,
    media_view_id   INTEGER             NOT NULL,
    hash        TEXT                NOT NULL,
    start       TEXT                NOT NULL,
    end       TEXT                NOT NULL,
    encryption_key       TEXT                ,
    FOREIGN KEY(media_view_id) REFERENCES media_view(id)
);

CREATE TABLE IF NOT EXISTS storage
(
    id          INTEGER PRIMARY KEY NOT NULL,
    hash        TEXT                NOT NULL,
    path       TEXT                NOT NULL
);

CREATE TABLE IF NOT EXISTS search_index
(
    id          INTEGER PRIMARY KEY NOT NULL,
    uuid   TEXT             NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS search_assoc
(
    id          INTEGER PRIMARY KEY NOT NULL,
    search_index_id   INTEGER             NOT NULL,
    srt_id   INTEGER             NOT NULL,
    FOREIGN KEY(search_index_id) REFERENCES search_index(id),
    FOREIGN KEY(srt_id) REFERENCES srtfile(id)
);
