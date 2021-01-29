use super::model2::*;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::Arc;

pub trait OsuBeatmapSets {
    fn beatmap_sets(&self) -> Vec<Box<dyn OsuBeatmapSet>>;
    fn boxed(self) -> Box<dyn OsuBeatmapSets>;
}

pub trait OsuBeatmapSet {
    fn beatmaps(&self) -> Vec<OsuBeatmapInfoHolder>;
    fn boxed(self) -> Box<dyn OsuBeatmapSet>;
}

#[derive(Debug, Clone, new)]
pub struct OsuBeatmapInfoHolder {
    pub ascii: BasicSongInfo,
    pub unicode: BasicSongInfo,
    pub beatmapset_id: u64,
    pub background: Option<PathBuf>,
    pub audio: PathBuf,
    pub beatmap: PathBuf,
    pub extensions: (Option<String>, Option<String>),
}

#[derive(Debug, Clone, new)]
pub struct OsuBeatmapInfoHolderSimple {
    pub info: BasicSongInfo,
    pub beatmapset_id: u64,
    pub background: Option<PathBuf>,
    pub audio: PathBuf,
    pub beatmap: PathBuf,
    pub extensions: (Option<String>, Option<String>),
}

impl From<(OsuBeatmapInfoHolder, bool)> for OsuBeatmapInfoHolderSimple {
    fn from((other, unicode_support): (OsuBeatmapInfoHolder, bool)) -> Self {
        Self::new(
            if unicode_support {
                other.unicode
            } else {
                other.ascii
            },
            other.beatmapset_id,
            other.background,
            other.audio,
            other.beatmap,
            other.extensions,
        )
    }
}

#[derive(Debug, Clone, new)]
pub struct OsuBeatmapInfoExtracted {
    pub ascii_opt: Option<BasicSongInfo>,
    pub unicode: BasicSongInfo,
    pub background: Option<String>,
    pub audio: String,
}

#[derive(Debug, Clone, new)]
pub struct BasicSongInfo {
    pub title: String,
    pub artist: String,
}

impl BasicSongInfo {
    pub fn filter_ascii(&self) -> Self {
        Self::new(
            self.title.chars().filter(char::is_ascii).collect(),
            self.artist.chars().filter(char::is_ascii).collect(),
        )
    }
}

impl TryFrom<&PathBuf> for OsuBeatmapInfoExtracted {
    type Error = String;
    fn try_from(path: &PathBuf) -> Result<Self, String> {
        Self::try_from(&std::fs::read_to_string(path).map_err(|e| format!("{:?}", e))?)
    }
}

fn assemble_hierarchy<T>(items: Vec<(bool, T)>) -> Vec<Vec<T>> {
    let mut nested: Vec<Vec<T>> = vec![];
    let mut buffer: Vec<T> = vec![];
    for (head, item) in items {
        if head {
            if !buffer.is_empty() {
                nested.push(buffer);
            }
            buffer = vec![];
        }
        buffer.push(item);
    }
    if !buffer.is_empty() {
        nested.push(buffer);
    }
    nested
}

fn get_osu_beatmap_sections(beatmap_string: &str) -> HashMap<String, Vec<String>> {
    let beatmap_string_unixnewlines = beatmap_string.replace('\r', "");
    let beatmap_lines: Vec<(bool, &str)> = beatmap_string_unixnewlines
        .split('\n')
        .map(|line| {
            let trimmed_line = line.trim();
            (
                trimmed_line.starts_with('[') && trimmed_line.ends_with(']'),
                line,
            )
        })
        .skip_while(|t| !t.0)
        .collect();
    let beatmap_sections_vec: Vec<Vec<&str>> = assemble_hierarchy(beatmap_lines);
    let beatmap_sections: HashMap<String, Vec<String>> = beatmap_sections_vec
        .iter()
        .map(|vec| {
            vec.iter()
                .filter(|line| !line.is_empty())
                .collect::<Vec<&&str>>()
        })
        .map(|vec| {
            let section_syntax = vec.first().unwrap().trim().to_string();
            let section_key = section_syntax[1..(section_syntax.len() - 1)]
                .trim()
                .to_string();
            (
                section_key.to_lowercase(),
                vec.iter().skip(1).map(|s| s.to_string()).collect(),
            )
        })
        .collect();
    beatmap_sections
}

impl TryFrom<&String> for OsuBeatmapInfoExtracted {
    type Error = String;
    fn try_from(beatmap_string: &String) -> Result<Self, String> {
        let beatmap_sections: HashMap<String, Vec<String>> =
            get_osu_beatmap_sections(beatmap_string);
        // println!("{:#?}", beatmap_sections);
        // println!("{:#?}", beatmap_sections.get("events"));
        let background: Option<String> = beatmap_sections
            .get("events")
            .and_then(|events_vec| {
                events_vec
                    .iter()
                    .filter(|line| line.starts_with("0,0,\""))
                    .map(|line| line.split('"').nth(1).unwrap())
                    .next()
            })
            .map(|item| item.to_string());
        let general: HashMap<String, String> = beatmap_sections
            .get("general")
            .unwrap()
            .iter()
            .filter(|line| line.contains(": "))
            .map(|line| {
                let mut spl = line.splitn(2, ": ");
                (
                    spl.next().unwrap().to_string(),
                    spl.next().unwrap().to_string(),
                )
            })
            .collect();
        // println!("{:#?}", general);
        let audio_filename = general.get("AudioFilename").unwrap();
        let metadata: HashMap<String, String> = beatmap_sections
            .get("metadata")
            .unwrap()
            .iter()
            .filter(|line| line.contains(':'))
            .map(|line| {
                let mut spl = line.splitn(2, ':');
                (
                    spl.next().unwrap().to_string(),
                    spl.next().unwrap().to_string(),
                )
            })
            .collect();
        // println!("{:#?}", metadata);
        let title = metadata.get("Title").unwrap();
        let artist = metadata.get("Artist").unwrap();
        let title_unicode_opt = metadata.get("TitleUnicode");
        let artist_unicode_opt = metadata.get("ArtistUnicode");
        let info_unknown = BasicSongInfo::new(title.to_string(), artist.to_string());
        let info_unicode_opt = title_unicode_opt.and_then(|title_unicode| {
            artist_unicode_opt.map(|artist_unicode| {
                BasicSongInfo::new(title_unicode.to_string(), artist_unicode.to_string())
            })
        });
        let (info_ascii, info_unicode) = if let Some(info_unicode_) = info_unicode_opt {
            (Some(info_unknown), info_unicode_)
        } else {
            (None, info_unknown)
        };
        // println!("{:#?}", title);
        // println!("{:#?}", title_unicode_opt);
        // println!("{:#?}", artist);
        // println!("{:#?}", artist_unicode_opt);
        // println!("{:#?}", audio_filename);
        // println!("{:#?}", background);
        // panic!();
        Ok(Self::new(
            info_ascii,
            info_unicode,
            background,
            audio_filename.to_string(),
        ))
    }
}

#[derive(Debug, Clone, new)]
pub struct Osu40BeatmapSetsReader {
    pub beatmapsets_folder: PathBuf,
}

#[derive(Debug, Clone, new)]
pub struct Osu50BeatmapSetsReader {
    pub hash_resolver: Arc<Osu50HashResolver>,
    pub connection: Arc<rusqlite::Connection>,
}

#[derive(Debug, Clone, new)]
pub struct Osu50HashResolver {
    pub folder: PathBuf,
}

impl Osu50HashResolver {
    pub fn resolve(&self, hash: &str) -> Result<PathBuf, String> {
        let final_path_buf = self
            .folder
            .join(hash.chars().take(1).collect::<String>())
            .join(hash.chars().take(2).collect::<String>())
            .join(hash);
        if final_path_buf.is_file() {
            Ok(final_path_buf)
        } else {
            Err(format!("{:?} is not a hashed file", final_path_buf))
        }
    }
}

impl TryFrom<&PathBuf> for Osu40BeatmapSetsReader {
    type Error = String;
    fn try_from(path: &PathBuf) -> Result<Self, String> {
        if !path.is_dir() {
            return Err(format!("{:?} is not a directory", path));
        }
        let songs_path = path.join("Songs");
        if !songs_path.is_dir() {
            return Err(format!(
                "{:?} directory was not found in your osu!classic directory",
                songs_path
            ));
        }
        Ok(Self::new(songs_path))
    }
}

impl OsuBeatmapSets for Osu40BeatmapSetsReader {
    fn boxed(self) -> Box<dyn OsuBeatmapSets> {
        Box::new(self)
    }
    fn beatmap_sets(&self) -> Vec<Box<dyn OsuBeatmapSet>> {
        let folders: Vec<PathBuf> = self
            .beatmapsets_folder
            .read_dir()
            .unwrap()
            .map(|entry_opt| entry_opt.unwrap())
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
                    .split(' ')
                    .next()
                    .unwrap()
                    .parse::<u64>()
                    .is_ok()
            })
            .collect();
        folders
            .iter()
            .map(|folder| Osu40BeatmapSet::new(folder.clone()))
            .map(|boxable| boxable.boxed())
            .collect()
    }
}

#[derive(Debug, Clone, new)]
pub struct Osu40BeatmapSet {
    pub beatmap_folder: PathBuf,
}

impl OsuBeatmapSet for Osu40BeatmapSet {
    fn boxed(self) -> Box<dyn OsuBeatmapSet> {
        Box::new(self)
    }
    fn beatmaps(&self) -> Vec<OsuBeatmapInfoHolder> {
        let beatmapset_id: u64 = self
            .beatmap_folder
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let osu_files: Vec<PathBuf> = self
            .beatmap_folder
            .read_dir()
            .unwrap()
            .map(|entry_opt| entry_opt.unwrap())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter(|path| {
                path.extension()
                    .map(|ext| ext.to_str().unwrap() == "osu")
                    .unwrap_or(false)
            })
            .collect();
        let osu_infos_extracted: Vec<(&PathBuf, OsuBeatmapInfoExtracted)> = osu_files
            .iter()
            .map(|path| (path, OsuBeatmapInfoExtracted::try_from(path).unwrap()))
            .collect();
        osu_infos_extracted
            .iter()
            .filter_map(
                |(path, beatmap_info): &(&PathBuf, OsuBeatmapInfoExtracted)| {
                    let background = beatmap_info.background.clone().and_then(|bkg| {
                        let bkg_test = self.beatmap_folder.join(bkg);
                        if bkg_test.is_file() {
                            Some(bkg_test)
                        } else {
                            None
                        }
                    });
                    let audio_opt = {
                        let aud = beatmap_info.audio.clone();
                        let aud_test = self.beatmap_folder.join(aud);
                        if aud_test.is_file() {
                            Some(aud_test)
                        } else {
                            None
                        }
                    };
                    let extensions = (
                        audio_opt.as_ref().and_then(|audio| {
                            PathBuf::from(&audio)
                                .extension()
                                .and_then(|x| x.to_str().map(|y| y.to_lowercase()))
                        }),
                        background.as_ref().and_then(|bkg| {
                            PathBuf::from(&bkg)
                                .extension()
                                .and_then(|x| x.to_str().map(|y| y.to_lowercase()))
                        }),
                    );
                    audio_opt.and_then(|audio| {
                        if audio.is_file() {
                            Some(OsuBeatmapInfoHolder::new(
                                beatmap_info
                                    .ascii_opt
                                    .clone()
                                    .unwrap_or_else(|| beatmap_info.unicode.filter_ascii()),
                                beatmap_info.unicode.clone(),
                                beatmapset_id,
                                background,
                                audio,
                                PathBuf::from(path),
                                extensions,
                            ))
                        } else {
                            None
                        }
                    })
                },
            )
            .collect()
    }
}

impl TryFrom<&PathBuf> for Osu50BeatmapSetsReader {
    type Error = String;
    fn try_from(path: &PathBuf) -> Result<Self, String> {
        if !path.is_dir() {
            return Err(format!("{:?} is not a directory", path));
        }
        let files_path = path.join("files");
        if !files_path.is_dir() {
            return Err(format!(
                "{:?} directory was not found in your osu!lazer directory",
                files_path
            ));
        }
        let client_path = path.join("client.db");
        if !client_path.is_file() {
            return Err(format!(
                "{:?} file was not found in your osu!lazer directory",
                files_path
            ));
        }
        let connection = rusqlite::Connection::open_with_flags(
            client_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|x| format!("{:?}", x))?;
        let mut connection_memory = rusqlite::Connection::open_in_memory().unwrap();
        rusqlite::backup::Backup::new(&connection, &mut connection_memory)
            .unwrap()
            .run_to_completion(100000, std::time::Duration::from_millis(0), None)
            .unwrap();
        Ok(Self::new(
            Arc::new(Osu50HashResolver::new(files_path)),
            Arc::new(connection_memory),
        ))
    }
}

impl OsuBeatmapSets for Osu50BeatmapSetsReader {
    fn boxed(self) -> Box<dyn OsuBeatmapSets> {
        Box::new(self)
    }
    fn beatmap_sets(&self) -> Vec<Box<dyn OsuBeatmapSet>> {
        let mut stmt = self
            .connection
            .prepare(PRP_STMT_OSU_LAZER_LIST_BEATMAPSETS)
            .unwrap();
        let beatmap_set_db_listing_item: Vec<Osu50BeatmapSetDbListingItem> = stmt
            .query_map(rusqlite::params![], |row| {
                Ok(Osu50BeatmapSetDbListingItem::new(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    (row.get(5)?, row.get(6)?),
                    (row.get(7)?, row.get(8)?),
                ))
            })
            .unwrap()
            .filter_map(|x| x.ok())
            .collect();
        beatmap_set_db_listing_item
            .into_iter()
            .map(|beatmapset_db_info| {
                Box::new(Osu50BeatmapSet::new(
                    self.hash_resolver.clone(),
                    self.connection.clone(),
                    beatmapset_db_info,
                )) as Box<dyn OsuBeatmapSet>
            })
            .collect()
    }
}

#[derive(Debug, Clone, new)]
pub struct Osu50BeatmapSet {
    pub hash_resolver: Arc<Osu50HashResolver>,
    pub connection: Arc<rusqlite::Connection>,
    pub beatmapset_db_info: Osu50BeatmapSetDbListingItem,
}

impl OsuBeatmapSet for Osu50BeatmapSet {
    fn boxed(self) -> Box<dyn OsuBeatmapSet> {
        Box::new(self)
    }
    fn beatmaps(&self) -> Vec<OsuBeatmapInfoHolder> {
        if let Some(audio) = self
            .beatmapset_db_info
            .audio
            .1
            .clone()
            .and_then(|hash| self.hash_resolver.resolve(&hash).ok())
        {
            let background = self
                .beatmapset_db_info
                .background
                .1
                .clone()
                .and_then(|hash| self.hash_resolver.resolve(&hash).ok());
            let beatmapset_id = self.beatmapset_db_info.id;
            let title = self.beatmapset_db_info.title.clone();
            let artist = self.beatmapset_db_info.artist.clone();
            let title_unicode_opt = self.beatmapset_db_info.title_unicode.clone();
            let artist_unicode_opt = self.beatmapset_db_info.artist_unicode.clone();
            let info_unknown = BasicSongInfo::new(title, artist);
            let info_unicode_opt = title_unicode_opt.and_then(|title_unicode| {
                artist_unicode_opt.map(|artist_unicode| {
                    BasicSongInfo::new(title_unicode.to_string(), artist_unicode)
                })
            });
            let (info_ascii, info_unicode) = if let Some(info_unicode_) = info_unicode_opt {
                (Some(info_unknown), info_unicode_)
            } else {
                (None, info_unknown)
            };
            let mut stmt = self
                .connection
                .prepare(PRP_STMT_OSU_LAZER_LIST_BEATMAPS_FROM_SET)
                .unwrap();
            let beatmap_from_set_db_listing_item: Vec<Osu50BeatmapDbListingItem> = stmt
                .query_map(rusqlite::params![beatmapset_id], |row| {
                    Ok(Osu50BeatmapDbListingItem::new(
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                    ))
                })
                .unwrap()
                .filter_map(|x| x.ok())
                .collect();
            beatmap_from_set_db_listing_item
                .into_iter()
                .map(|osu_betmap_db_listing| {
                    let cloned_info_ascii = info_ascii.clone();
                    let cloned_info_unicode = info_unicode.clone();
                    let cloned_background = background.clone();
                    let cloned_audio = audio.clone();
                    let cloned_background_0 = self.beatmapset_db_info.background.0.clone();
                    let cloned_audio_0 = self.beatmapset_db_info.audio.0.clone();
                    self.hash_resolver
                        .resolve(&osu_betmap_db_listing.hash)
                        .map(|beatmap_pathbuf| {
                            OsuBeatmapInfoHolder::new(
                                cloned_info_ascii
                                    .unwrap_or_else(|| cloned_info_unicode.filter_ascii()),
                                cloned_info_unicode,
                                beatmapset_id as u64,
                                cloned_background,
                                cloned_audio,
                                beatmap_pathbuf,
                                (
                                    cloned_audio_0.clone().and_then(|aud| {
                                        PathBuf::from(&aud).extension().and_then(|x| {
                                            x.to_str().map(|y| y.to_lowercase())
                                        })
                                    }),
                                    cloned_background_0.clone().and_then(|bkg| {
                                        PathBuf::from(&bkg).extension().and_then(|x| {
                                            x.to_str().map(|y| y.to_lowercase())
                                        })
                                    }),
                                ),
                            )
                        })
                        .ok()
                })
                .filter_map(|x| x)
                .collect()
        } else {
            vec![]
        }
    }
}

impl OsuBeatmapInfoHolderSimple {
    pub fn build_path(&self, path: &PathBuf, filename_template: &str) -> PathBuf {
        let mut filename: String = "".to_string();
        let mut shift = false;
        for ch in filename_template.chars() {
            if shift {
                shift = false;
                match ch {
                    'a' => filename.push_str(&self.info.artist),
                    't' => filename.push_str(&self.info.title),
                    'i' => filename.push_str(&format!("{}", self.beatmapset_id)),
                    '/' => (),
                    _ => filename.push(ch),
                }
            } else if ch == '%' {
                shift = true;
            } else if ch == '/' {
            } else {
                filename.push(ch);
            }
        }
        if shift {
            filename.push('%');
        }
        for ch in &['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\''] {
            filename = filename.replace(*ch, "");
        }
        if let (Some(audio_extension), _) = &self.extensions {
            filename.push('.');
            filename.push_str(&audio_extension);
        }
        path.join(filename)
    }
}
