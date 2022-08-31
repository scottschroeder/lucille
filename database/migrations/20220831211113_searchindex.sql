-- Add migration script here

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
