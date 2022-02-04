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
use std::convert::TryFrom;
use std::path::PathBuf;

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
        .filter(|x| x.is_ok())
        .map(|x| x.as_ref().unwrap())
        .collect();
    let beatmap_set_readers_failures: Vec<&String> = beatmap_set_readers
        .iter()
        .filter(|x| x.is_err())
        .map(|x| x.as_ref().map(|_| "").unwrap_err())
        .collect();
    for beatmap_set_readers_failure in beatmap_set_readers_failures {
        eprintln!("WARN: {}", beatmap_set_readers_failure);
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
    std::fs::create_dir_all(&cli_args.songs_destination).unwrap();
    let beatmap_copies: Vec<(PathBuf, &OsuBeatmapInfoHolderSimple)> = beatmap_infos
        .iter()
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

    let thread_pool = threadpool::ThreadPool::new(16);
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
                .arg(&beatmap_info_holder.audio.to_str().unwrap())
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
            .arg(&destination_path.to_str().unwrap())
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
        if let Ok(mut tag) = audiotags::Tag::new().read_from_path(&destination_path) {
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
                    let reader_image_result: Result<image::io::Reader<_>, _> =
                        image::io::Reader::open(background_source_path);
                    // reader_image_result.as_ref().unwrap();
                    if let Ok(reader_image) = reader_image_result {
                        let reader_image_with_guess: image::io::Reader<_> = match guessed_format {
                            Some(x) => image::io::Reader::with_format(reader_image.into_inner(), x),
                            None => reader_image,
                        };
                        let loaded_image_option: Result<image::DynamicImage, _> =
                            reader_image_with_guess.decode();
                        // loaded_image_option.as_ref().unwrap();
                        if let Ok(loaded_image) = loaded_image_option {
                            let thumbnail = loaded_image.thumbnail(1024, 1024);
                            let mut bytes: Vec<u8> = Vec::new();
                            thumbnail
                                .write_to(&mut bytes, image::ImageOutputFormat::Bmp)
                                .unwrap();
                            {
                                let cover = audiotags::Picture {
                                    mime_type: audiotags::MimeType::Bmp,
                                    data: &bytes,
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
