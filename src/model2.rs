pub const PRP_STMT_OSU_LAZER_LIST_BEATMAPSETS: &str = r#"
SELECT DISTINCT
    BeatmapSetInfo.OnlineBeatmapSetID,
    BeatmapMetadata.Title,
    BeatmapMetadata.Artist,
    BeatmapMetadata.TitleUnicode,
    BeatmapMetadata.ArtistUnicode,
    BeatmapMetadata.BackgroundFile,
    (
    	SELECT DISTINCT
			Hash
		FROM
			FileInfo
		INNER JOIN
			BeatmapSetFileInfo
		ON
			(FileInfo.ID = BeatmapSetFileInfo.FileInfoID)
		WHERE
			BeatmapSetFileInfo.BeatmapSetInfoID = BeatmapSetInfo.ID
		AND
		    BeatmapSetFileInfo.Filename = BeatmapMetadata.BackgroundFile
	) AS BackgroundHash,
    BeatmapMetadata.AudioFile,
    (
    	SELECT DISTINCT
			Hash
		FROM
			FileInfo
		INNER JOIN
			BeatmapSetFileInfo
		ON
			(FileInfo.ID = BeatmapSetFileInfo.FileInfoID)
		WHERE
			BeatmapSetFileInfo.BeatmapSetInfoID = BeatmapSetInfo.ID
		AND
		    BeatmapSetFileInfo.Filename = BeatmapMetadata.AudioFile 
	) AS AudioHash
FROM
    BeatmapSetInfo
INNER JOIN
    BeatmapMetadata
ON
    (BeatmapSetInfo.MetadataID = BeatmapMetadata.ID)
WHERE
    BeatmapSetInfo.OnlineBeatmapSetID IS NOT NULL
"#;

#[derive(Debug, Clone, new)]
pub struct Osu50BeatmapSetDbListingItem {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub title_unicode: Option<String>,
    pub artist_unicode: Option<String>,
    pub background: (Option<String>, Option<String>),
    pub audio: (Option<String>, Option<String>),
}

pub const PRP_STMT_OSU_LAZER_LIST_BEATMAPS_FROM_SET: &str = r#"
SELECT DISTINCT
	BeatmapSetInfo.OnlineBeatmapSetID,
	BeatmapInfo.OnlineBeatmapID,
	BeatmapInfo.Path,
    BeatmapInfo.Hash
FROM
	BeatmapInfo
INNER JOIN
	BeatmapSetInfo
ON
    (BeatmapInfo.BeatmapSetInfoID = BeatmapSetInfo.ID)
WHERE
    BeatmapSetInfo.OnlineBeatmapSetID = ?1
"#;

#[derive(Debug, Clone, new)]
pub struct Osu50BeatmapDbListingItem {
    pub set_id: i64,
    pub id: i64,
    pub path: String,
    pub hash: String,
}
