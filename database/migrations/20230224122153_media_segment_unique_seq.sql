-- Add migration script here


PRAGMA foreign_keys = 0;
CREATE TABLE migration_tmp_table AS SELECT * FROM media_segment;
DROP TABLE media_segment;
CREATE TABLE media_segment (
    id          INTEGER PRIMARY KEY NOT NULL,
    media_view_id   INTEGER             NOT NULL,
    hash        TEXT                NOT NULL CHECK(hash <> ''),
    seq_id      INTEGER             NOT NULL,
    start       TEXT                NOT NULL CHECK(start <> ''),
    encryption_key       TEXT             CHECK(encryption_key <> ''),
    FOREIGN KEY(media_view_id) REFERENCES media_view(id),
    CONSTRAINT "Unique Sequence Per View" UNIQUE (media_view_id, seq_id)
);
INSERT INTO media_segment SELECT * FROM migration_tmp_table;
DROP TABLE migration_tmp_table;
PRAGMA foreign_keys = 1;
