-- Add migration script here

PRAGMA defer_foreign_keys = ON;

CREATE TABLE migration_tmp_table AS SELECT * FROM chapter;
DROP TABLE chapter;
CREATE TABLE chapter (
    id          INTEGER PRIMARY KEY NOT NULL,
    corpus_id   INTEGER             NOT NULL,
    title       TEXT                NOT NULL CHECK(title <> ''),
    season      INTEGER,
    episode     INTEGER,
    hash        TEXT                NOT NULL UNIQUE CHECK(hash <> ''),
    FOREIGN KEY(corpus_id) REFERENCES corpus(id) ON DELETE CASCADE
);
INSERT INTO chapter SELECT * FROM migration_tmp_table;
DROP TABLE migration_tmp_table;

CREATE TABLE migration_tmp_table AS SELECT * FROM srtfile;
DROP TABLE srtfile;
CREATE TABLE srtfile (
    id          INTEGER PRIMARY KEY NOT NULL,
    uuid        TEXT NOT            NULL UNIQUE,
    chapter_id   INTEGER             NOT NULL,
    data    BLOB NOT NULL,
    FOREIGN KEY(chapter_id) REFERENCES chapter(id) ON DELETE CASCADE
);
INSERT INTO srtfile SELECT * FROM migration_tmp_table;
DROP TABLE migration_tmp_table;

CREATE TABLE migration_tmp_table AS SELECT * FROM media_view;
DROP TABLE media_view;
CREATE TABLE media_view (
    id          INTEGER PRIMARY KEY NOT NULL,
    chapter_id   INTEGER             NOT NULL,
    name       TEXT                NOT NULL CHECK(name <> ''),

    FOREIGN KEY(chapter_id) REFERENCES chapter(id) ON DELETE CASCADE,
    UNIQUE(chapter_id, name)
);
INSERT INTO media_view SELECT * FROM migration_tmp_table;
DROP TABLE migration_tmp_table;

CREATE TABLE migration_tmp_table AS SELECT * FROM search_assoc;
DROP TABLE search_assoc;
CREATE TABLE search_assoc (
    id          INTEGER PRIMARY KEY NOT NULL,
    search_index_id   INTEGER             NOT NULL,
    srt_id   INTEGER             NOT NULL,
    FOREIGN KEY(search_index_id) REFERENCES search_index(id) ON DELETE CASCADE,
    FOREIGN KEY(srt_id) REFERENCES srtfile(id) -- choosing not to delete in this direction
);
INSERT INTO search_assoc SELECT * FROM migration_tmp_table;
DROP TABLE migration_tmp_table;

CREATE TABLE migration_tmp_table AS SELECT * FROM media_segment;
DROP TABLE media_segment;
CREATE TABLE media_segment (
    id          INTEGER PRIMARY KEY NOT NULL,
    media_view_id   INTEGER             NOT NULL,
    hash        TEXT                NOT NULL CHECK(hash <> ''),
    seq_id      INTEGER             NOT NULL,
    start       TEXT                NOT NULL CHECK(start <> ''),
    encryption_key       TEXT             CHECK(encryption_key <> ''),
    FOREIGN KEY(media_view_id) REFERENCES media_view(id) ON DELETE CASCADE,
    CONSTRAINT "Unique Sequence Per View" UNIQUE (media_view_id, seq_id)
);
INSERT INTO media_segment SELECT * FROM migration_tmp_table;
DROP TABLE migration_tmp_table;

PRAGMA defer_foreign_keys = OFF;
