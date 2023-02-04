-- Add migration script here
CREATE TABLE IF NOT EXISTS corpus
(
    id          INTEGER PRIMARY KEY NOT NULL,
    title       TEXT                NOT NULL UNIQUE CHECK(title <> '')
);

CREATE TABLE IF NOT EXISTS chapter
(
    id          INTEGER PRIMARY KEY NOT NULL,
    corpus_id   INTEGER             NOT NULL,
    title       TEXT                NOT NULL CHECK(title <> ''),
    season      INTEGER,
    episode     INTEGER,
    hash        TEXT                NOT NULL UNIQUE CHECK(hash <> ''),
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
    name       TEXT                NOT NULL CHECK(name <> ''),

    FOREIGN KEY(chapter_id) REFERENCES chapter(id),
    UNIQUE(chapter_id, name)
);

CREATE TABLE IF NOT EXISTS media_segment
(
    id          INTEGER PRIMARY KEY NOT NULL,
    media_view_id   INTEGER             NOT NULL,
    hash        TEXT                NOT NULL CHECK(hash <> ''),
    seq_id      INTEGER             NOT NULL,
    start       TEXT                NOT NULL CHECK(start <> ''),
    encryption_key       TEXT             CHECK(encryption_key <> '')   ,
    FOREIGN KEY(media_view_id) REFERENCES media_view(id)
);

CREATE TABLE IF NOT EXISTS storage
(
    id          INTEGER PRIMARY KEY NOT NULL,
    hash        TEXT                NOT NULL CHECK(hash <> ''),
    path       TEXT                NOT NULL CHECK(path <> '') UNIQUE
);

CREATE TABLE IF NOT EXISTS search_index
(
    id          INTEGER PRIMARY KEY NOT NULL,
    uuid   TEXT             NOT NULL UNIQUE CHECK(uuid <> '')
);

CREATE TABLE IF NOT EXISTS search_assoc
(
    id          INTEGER PRIMARY KEY NOT NULL,
    search_index_id   INTEGER             NOT NULL,
    srt_id   INTEGER             NOT NULL,
    FOREIGN KEY(search_index_id) REFERENCES search_index(id),
    FOREIGN KEY(srt_id) REFERENCES srtfile(id)
);
