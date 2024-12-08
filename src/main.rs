// #[macro_use]
// extern crate derivative;
#[macro_use]
extern crate derive_new;
// #[macro_use]
// extern crate derive_more;

mod cli;
mod model;
mod model2;

use self::cli::*;
use self::model::*;
use core::panic;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync;
use std::sync::Arc;

type FnBeatmapSetReader = dyn Fn(&PathBuf) -> Result<Box<dyn OsuBeatmapSets>, String>;

fn main() -> Result<(), String> {
    let cli_args = get_arguments_parsed();
    if !cli_args.osu_source.is_dir() {
        return Err(format!("Path {:?} is not a directory", cli_args.osu_source));
    }
    let beatmap_set_readers_fns: Vec<&FnBeatmapSetReader> = vec![
        &(|x| Osu40BeatmapSetsReader::try_from(x).map(|a| a.boxed())),
        &(|x| Osu50BeatmapSetsReader::try_from(x).map(|a| a.boxed())),
    ];
    let beatmap_set_readers: Vec<Result<Box<dyn OsuBeatmapSets>, String>> = beatmap_set_readers_fns
        .iter()
        .map(|f| f(&cli_args.osu_source))
        .collect();
    let beatmap_set_readers_success: Vec<&Box<dyn OsuBeatmapSets>> = beatmap_set_readers
        .iter()
        .filter_map(|x| x.as_ref().ok())
        .collect();
    let beatmap_set_readers_failures: Vec<&String> = beatmap_set_readers
        .iter()
        .filter(|x| x.is_err())
        .map(|x| x.as_ref().map(|_| "").unwrap_err())
        .collect();
    if beatmap_set_readers_success.is_empty() {
        for beatmap_set_readers_failure in beatmap_set_readers_failures {
            eprintln!("WARN: {}", beatmap_set_readers_failure);
        }
    }
    let beatmap_set_reader = beatmap_set_readers_success.first().ok_or_else(|| {
        format!(
            "No healthy osu! folder structure identified at {:?}",
            &cli_args.osu_source
        )
    })?;
    let beatmap_sets_vec: Vec<Box<dyn OsuBeatmapSet>> = beatmap_set_reader.beatmap_sets();
    let beatmap_info_vec_vec: Vec<Vec<OsuBeatmapInfoHolder>> = beatmap_sets_vec
        .iter()
        .map(|beatmap_set| beatmap_set.beatmaps())
        .collect();
    let beatmap_info_option_vec: Vec<Option<&OsuBeatmapInfoHolder>> = beatmap_info_vec_vec
        .iter()
        .map(|beatmap_infos: &Vec<OsuBeatmapInfoHolder>| beatmap_infos.first())
        .collect();
    let beatmap_infos: Vec<OsuBeatmapInfoHolderSimple> = beatmap_info_option_vec
        .iter()
        .filter_map(|beatmap_info_opt: &Option<&OsuBeatmapInfoHolder>| beatmap_info_opt.as_ref())
        .map(|beatmap_info: &&OsuBeatmapInfoHolder| {
            OsuBeatmapInfoHolderSimple::from(((*beatmap_info).clone(), cli_args.unicode_filename))
        })
        .collect();
    let deduped_beatmap_infos = if cli_args.duplicated {
        beatmap_infos
    } else {
        deduplicate_infos(&beatmap_infos)
    };
    std::fs::create_dir_all(&cli_args.songs_destination).unwrap();
    let beatmap_copies: Vec<(PathBuf, OsuBeatmapInfoHolderSimple)> = deduped_beatmap_infos
        .into_iter()
        .map(|x| {
            (
                x.build_path(&cli_args.songs_destination, &cli_args.filename_template),
                x,
            )
        })
        .collect();
    if cli_args.remove_missing_songs {
        let beatmap_files: Vec<PathBuf> = beatmap_copies
            .iter()
            .map(|(path, _)| path.clone())
            .collect();
        let existing_files: Vec<PathBuf> = cli_args
            .songs_destination
            .read_dir()
            .unwrap()
            .filter_map(|x| x.ok())
            .map(|entry| entry.path())
            .collect();
        let files_to_remove: Vec<&PathBuf> = existing_files
            .iter()
            .filter(|path| !beatmap_files.contains(path))
            .collect();
        for file in files_to_remove {
            std::fs::remove_file(file).unwrap();
        }
    }

    let thread_pool = threadpool::ThreadPool::new(
        std::thread::available_parallelism()
            .and_then(|x| Ok(x.get()))
            .unwrap_or(2)
            * 2,
    );
    for (destination_path, beatmap_info_holder) in beatmap_copies.into_iter() {
        let cli_args_cloned = cli_args.clone();
        let beatmap_info_holder_cloned = beatmap_info_holder.clone();
        thread_pool.execute(move || {
            do_copy(
                destination_path,
                beatmap_info_holder_cloned,
                cli_args_cloned,
            )
        });
    }
    thread_pool.join();
    Ok(())
}

fn do_copy(
    mut destination_path: PathBuf,
    beatmap_info_holder: OsuBeatmapInfoHolderSimple,
    cli_args: cli::CliArguments,
) {
    let compressing = cli_args.compress >= 0 && cli_args.compress <= 9;
    if compressing {
        destination_path.set_extension("mp3");
    }
    if !destination_path.is_file() || cli_args.remove_missing_songs {
        {
            let sps = subprocess::Exec::cmd("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg(beatmap_info_holder.audio.to_str().unwrap())
                .arg("-map")
                .arg("0:a")
                .arg("-c:a");
            match compressing {
                false => sps.arg("copy"),
                true => sps
                    .arg("libmp3lame")
                    .arg("-q:a")
                    .arg(std::ffi::OsStr::new(cli_args.compress.to_string().as_str())),
            }
            .arg(destination_path.to_str().unwrap())
            .stdout(subprocess::Redirection::Pipe)
            .stderr(subprocess::Redirection::Pipe)
            .join()
            .unwrap();
        }
        // std::fs::write(
        //     &destination_path,
        //     &std::fs::read(&beatmap_info_holder.audio).unwrap(),
        // )
        // .unwrap();
        let destination_path_clone: PathBuf = destination_path.clone();
        let beatmap_info_holder_clone: OsuBeatmapInfoHolderSimple = beatmap_info_holder;
        let skip_info = cli_args.skip_info;
        let skip_bitmap = cli_args.skip_bitmap;
        let thread_pool = threadpool::ThreadPool::new(1);
        thread_pool.execute(move || {
            update_audio_metadata(
                &destination_path_clone,
                &beatmap_info_holder_clone,
                skip_info,
                skip_bitmap,
            )
        });
        thread_pool.join();
    }
}

fn update_audio_metadata(
    destination_path: &PathBuf,
    beatmap_info_holder: &OsuBeatmapInfoHolderSimple,
    skip_info: bool,
    skip_pic: bool,
) {
    if !skip_info {
        if let Ok(mut tag) = audiotags::Tag::new().read_from_path(destination_path) {
            tag.remove_album();
            tag.remove_album_artist();
            tag.remove_album_cover();
            tag.remove_album_title();
            tag.remove_artist();
            tag.remove_disc();
            tag.remove_disc_number();
            tag.remove_title();
            tag.remove_total_discs();
            tag.remove_total_tracks();
            tag.remove_track();
            tag.remove_track_number();
            tag.remove_year();
            tag.set_album_title("osu!");
            // tag.set_text(format!(
            //     "https://osu.ppy.sh/beatmapsets/{}",
            //     beatmap_info_holder.beatmapset_id
            // ));
            tag.set_title(&beatmap_info_holder.info.title);
            tag.set_artist(&beatmap_info_holder.info.artist);
            if !skip_pic {
                if let Some(background_source_path) = &beatmap_info_holder.background {
                    let guessed_format: Option<image::ImageFormat> = beatmap_info_holder
                        .extensions
                        .1
                        .clone()
                        .and_then(image::ImageFormat::from_extension);
                    let reader_image_result: Result<image::ImageReader<_>, _> =
                        image::ImageReader::open(background_source_path);
                    // reader_image_result.as_ref().unwrap();
                    if let Ok(reader_image) = reader_image_result {
                        let reader_image_with_guess: image::ImageReader<_> = match guessed_format {
                            Some(x) => {
                                image::ImageReader::with_format(reader_image.into_inner(), x)
                            }
                            None => reader_image,
                        };
                        let loaded_image_option: Result<image::DynamicImage, _> =
                            reader_image_with_guess.decode();
                        // loaded_image_option.as_ref().unwrap();
                        if let Ok(loaded_image) = loaded_image_option {
                            let thumbnail = loaded_image.thumbnail(1024, 1024);
                            let mut bytes_cursor = std::io::Cursor::new(vec![]);
                            thumbnail
                                .write_to(&mut bytes_cursor, image::ImageFormat::Png)
                                .unwrap();
                            {
                                let cover = audiotags::Picture {
                                    mime_type: audiotags::MimeType::Png,
                                    data: bytes_cursor.get_ref(),
                                };
                                tag.set_album_cover(cover.clone());
                            }
                        }
                    }
                }
            }
            tag.write_to_path(destination_path.to_str().unwrap())
                .unwrap_or(());
        }
    }
}

fn ffprobe_audio_duration(file: &PathBuf) -> Option<FFProbeAudioStream> {
    subprocess::Exec::cmd("ffprobe")
        .arg("-hide_banner")
        .arg("-show_format")
        .arg("-show_streams")
        .arg("-count_frames")
        .arg("-count_packets")
        .arg("-output_format")
        .arg("json")
        .arg(&file)
        .stdout(subprocess::Redirection::Pipe)
        .stderr(subprocess::Redirection::Pipe)
        .capture()
        .ok()
        .and_then(|capture_data| match capture_data.exit_status {
            subprocess::ExitStatus::Exited(0) => {
                let ffpo = serde_json::from_slice::<FFProbeOutput>(&capture_data.stdout).unwrap();
                let audstr = ffpo
                    .streams
                    .iter()
                    .map(|i| match i {
                        FFProbeStream::Audio(a) => Some(a),
                        _ => None,
                    })
                    .flatten()
                    .next()?;
                Some((*audstr).clone())
            }
            _ => None,
        })
}

fn deduplicate_infos(duplicated: &[OsuBeatmapInfoHolderSimple]) -> Vec<OsuBeatmapInfoHolderSimple> {
    let mut ffpas_map = HashMap::<PathBuf, FFProbeAudioStream>::new();
    {
        let (tx, rx) = std::sync::mpsc::channel::<(PathBuf, FFProbeAudioStream)>();
        {
            let tp = threadpool::ThreadPool::new(
                std::thread::available_parallelism()
                    .and_then(|x| Ok(x.get()))
                    .unwrap_or(2)
                    * 2,
            );
            for bm in duplicated.iter() {
                let aud = bm.audio.clone();
                let txc = tx.clone();
                tp.execute(move || {
                    if let Some(ffpas) = ffprobe_audio_duration(&aud) {
                        txc.send((aud, ffpas)).unwrap()
                    }
                    drop(txc);
                    ()
                });
            }
            tp.join();
            drop(tx);
            drop(tp);
        }
        while let Ok((k, v)) = rx.recv() {
            ffpas_map.insert(k, v);
        }
    }
    let comparables: Vec<_> = duplicated
        .iter()
        .map(|info| {
            let ffpas = ffpas_map.get(&info.audio)?;
            let bit_rate = ffpas.bit_rate.parse::<u32>().ok()?;
            let sample_rate = ffpas.sample_rate.parse::<u32>().ok()?;
            let audio_format = ffpas.codec_name;
            let mut time_base_part = ffpas.time_base.split('/');
            let time_base_up = time_base_part.next()?.parse::<f64>().ok()?;
            let time_base_dw = time_base_part.next()?.parse::<f64>().ok()?;
            let duration_sec = (ffpas.duration_ts as f64) * time_base_up / time_base_dw;
            Some(OsuBeatmapTrackInfo::new(
                info.clone(),
                duration_sec,
                bit_rate,
                sample_rate,
                audio_format,
            ))
        })
        .flatten()
        .collect();
    let mut groups: Vec<Vec<&OsuBeatmapTrackInfo>> = vec![];
    for item in comparables.iter() {
        let mut belongs_to: Vec<usize> = vec![];
        for (groupid, group) in groups.iter().enumerate() {
            for sample in group.iter() {
                if ((item.info.info_pair.ascii.title.to_lowercase()
                    == sample.info.info_pair.ascii.title.to_lowercase()
                    && item.info.info_pair.ascii.artist.to_lowercase()
                        == sample.info.info_pair.ascii.artist.to_lowercase())
                    || (item.info.info_pair.ascii.title.to_lowercase()
                        == sample.info.info_pair.ascii.title.to_lowercase()
                        && item.info.info_pair.ascii.artist.to_lowercase()
                            == sample.info.info_pair.ascii.artist.to_lowercase()))
                    && !belongs_to.contains(&groupid)
                {
                    belongs_to.push(groupid);
                }
            }
        }
        match belongs_to.len() {
            0 => groups.push(vec![item]),
            1 => groups[belongs_to[0]].push(item),
            _ => {
                let mut newgroup = vec![item];
                belongs_to.sort();
                belongs_to.reverse();
                for x in belongs_to {
                    newgroup.append(&mut groups.remove(x));
                }
                groups.push(newgroup)
            }
        }
    }
    let chosens: Vec<_> = groups
        .iter()
        .map(|group| match group.len() {
            0 => None,
            1 => Some(group[0].info.clone()),
            _ => {
                let mut cgroup = group.clone();
                cgroup.sort_by(|x, y| {
                    if y.info.beatmapset_id < x.info.beatmapset_id {
                        Ordering::Less
                    } else if y.info.beatmapset_id == x.info.beatmapset_id {
                        Ordering::Equal
                    } else {
                        Ordering::Greater
                    }
                });
                let mut group_iter = cgroup.iter();
                let mut best = group_iter.next().unwrap();
                while let Some(candidate) = group_iter.next() {
                    let bbrsc = (best.audio_bitrate as u64)
                        * match best.audio_format {
                            FFProbeAudioStreamCodec::MP3 => 8,
                            FFProbeAudioStreamCodec::VORBIS => 10,
                        };
                    let cbrsc = (candidate.audio_bitrate as u64)
                        * match candidate.audio_format {
                            FFProbeAudioStreamCodec::MP3 => 8,
                            FFProbeAudioStreamCodec::VORBIS => 10,
                        };
                    if bbrsc < cbrsc {
                        best = candidate;
                    } else if bbrsc == cbrsc
                        && best.info.beatmapset_id < candidate.info.beatmapset_id
                    {
                        best = candidate;
                    }
                }
                let latest_background =
                    cgroup.iter().filter(|x| x.info.background.is_some()).next();
                let best_mix = OsuBeatmapInfoHolderSimple::new(
                    best.info.info.clone(),
                    best.info.info_pair.clone(),
                    latest_background
                        .and_then(|x| Some(x.info.beatmapset_id))
                        .unwrap_or(best.info.beatmapset_id),
                    latest_background.and_then(|x| x.info.background.clone()),
                    best.info.audio.clone(),
                    best.info.beatmap.clone(),
                    (
                        best.info.extensions.0.clone(),
                        latest_background.and_then(|x| x.info.extensions.1.clone()),
                    ),
                );
                Some(best_mix)
            }
        })
        .flatten()
        .collect();
    chosens
}
