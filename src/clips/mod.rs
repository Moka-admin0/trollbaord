mod wav;

pub use wav::read_mono;

use crate::win32;
use anyhow::{Context, Result, bail, ensure};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const ORDER_FILE: &str = "order.txt";
const RECORDING_PREFIX: &str = "clip_";

pub fn default_folder() -> PathBuf {
    if let Some(folder) = std::env::var_os("TROLLBOARD_CLIPS") {
        return folder.into();
    }
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join("Music").join("trollboard")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub file_name: String,
    pub name: String,
    pub seconds: f32,
}

pub struct Library {
    folder: PathBuf,
    clips: Mutex<Vec<Clip>>,
}

impl Library {
    pub fn open(folder: PathBuf) -> Self {
        let library = Self { folder, clips: Mutex::new(Vec::new()) };
        library.refresh();
        library
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn clips(&self) -> Vec<Clip> {
        self.clips.lock().unwrap().clone()
    }

    pub fn get(&self, index: usize) -> Option<Clip> {
        self.clips.lock().unwrap().get(index).cloned()
    }

    pub fn len(&self) -> usize {
        self.clips.lock().unwrap().len()
    }

    pub fn path_of(&self, clip: &Clip) -> PathBuf {
        self.folder.join(&clip.file_name)
    }

    pub fn refresh(&self) {
        let mut clips = self.clips.lock().unwrap();
        *clips = self.scan(&clips);
    }

    pub fn save_recording(&self, samples: &[f32], sample_rate: u32) -> Result<Clip> {
        fs::create_dir_all(&self.folder)?;
        let mut clips = self.clips.lock().unwrap();
        *clips = self.scan(&clips);

        let file_name = self.unused_file_name(&next_recording_name(&clips));
        wav::write_mono(&self.folder.join(&file_name), samples, sample_rate)?;

        let clip = Clip { name: stem(&file_name), file_name, seconds: samples.len() as f32 / sample_rate as f32 };
        clips.push(clip.clone());
        self.write_order(&clips)?;
        Ok(clip)
    }

    pub fn import(&self, source: &Path) -> Result<Clip> {
        ensure!(wav::is_wav(source), "only .wav files can be added");
        wav::read_mono(source).context("not a readable WAV file")?;
        fs::create_dir_all(&self.folder)?;
        let mut clips = self.clips.lock().unwrap();
        *clips = self.scan(&clips);

        let source_name = sanitize(&source.file_stem().unwrap_or_default().to_string_lossy());
        let file_name = self.unused_file_name(if source_name.is_empty() { "sound" } else { &source_name });
        let destination = self.folder.join(&file_name);
        fs::copy(source, &destination)?;

        let seconds = wav::duration_seconds(&destination).unwrap_or(0.0);
        let clip = Clip { name: stem(&file_name), file_name, seconds };
        clips.push(clip.clone());
        self.write_order(&clips)?;
        Ok(clip)
    }

    pub fn rename(&self, index: usize, new_name: &str) -> Result<()> {
        let name = sanitize(new_name);
        ensure!(!name.is_empty(), "the name can't be empty");

        let mut clips = self.clips.lock().unwrap();
        let clip = clips.get_mut(index).context("that clip is gone")?;
        let file_name = format!("{name}.wav");
        if file_name == clip.file_name {
            return Ok(());
        }

        let only_case_changes = file_name.eq_ignore_ascii_case(&clip.file_name);
        if !only_case_changes && self.folder.join(&file_name).exists() {
            bail!("there's already a clip called \"{name}\"");
        }

        fs::rename(self.folder.join(&clip.file_name), self.folder.join(&file_name))?;
        clip.file_name = file_name;
        clip.name = name;
        self.write_order(&clips)
    }

    pub fn move_clip(&self, from: usize, to: usize) -> Result<()> {
        let mut clips = self.clips.lock().unwrap();
        if from == to || from >= clips.len() || to >= clips.len() {
            return Ok(());
        }
        let clip = clips.remove(from);
        clips.insert(to, clip);
        self.write_order(&clips)
    }

    pub fn delete(&self, index: usize) -> Result<()> {
        let mut clips = self.clips.lock().unwrap();
        let clip = clips.get(index).context("that clip is gone")?;
        win32::move_to_recycle_bin(&self.folder.join(&clip.file_name))?;
        clips.remove(index);
        self.write_order(&clips)
    }

    fn scan(&self, known: &[Clip]) -> Vec<Clip> {
        let files = self.wav_files();
        let mut ordered: Vec<String> = Vec::new();
        for file in self.saved_order().into_iter().chain(files.iter().cloned()) {
            if files.contains(&file) && !ordered.contains(&file) {
                ordered.push(file);
            }
        }

        ordered
            .into_iter()
            .map(|file_name| {
                let seconds = match known.iter().find(|clip| clip.file_name == file_name) {
                    Some(clip) => clip.seconds,
                    None => wav::duration_seconds(&self.folder.join(&file_name)).unwrap_or(0.0),
                };
                Clip { name: stem(&file_name), file_name, seconds }
            })
            .collect()
    }

    fn wav_files(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return Vec::new();
        };
        let mut files: Vec<String> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|file| wav::is_wav(Path::new(file)))
            .collect();
        files.sort_by_key(|file| file.to_lowercase());
        files
    }

    fn saved_order(&self) -> Vec<String> {
        let order = fs::read_to_string(self.folder.join(ORDER_FILE)).unwrap_or_default();
        order.lines().map(str::to_owned).collect()
    }

    fn write_order(&self, clips: &[Clip]) -> Result<()> {
        let lines: Vec<&str> = clips.iter().map(|clip| clip.file_name.as_str()).collect();
        fs::write(self.folder.join(ORDER_FILE), lines.join("\n"))?;
        Ok(())
    }

    fn unused_file_name(&self, stem: &str) -> String {
        let mut file_name = format!("{stem}.wav");
        let mut copy = 2;
        while self.folder.join(&file_name).exists() {
            file_name = format!("{stem} ({copy}).wav");
            copy += 1;
        }
        file_name
    }
}

fn next_recording_name(clips: &[Clip]) -> String {
    let highest = clips
        .iter()
        .filter_map(|clip| clip.name.strip_prefix(RECORDING_PREFIX)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{RECORDING_PREFIX}{:03}", highest + 1)
}

fn stem(file_name: &str) -> String {
    Path::new(file_name).file_stem().unwrap_or_default().to_string_lossy().into_owned()
}

fn sanitize(name: &str) -> String {
    const NOT_ALLOWED_IN_FILE_NAMES: &str = r#"<>:"/\|?*"#;
    let cleaned: String = name.chars().filter(|c| !NOT_ALLOWED_IN_FILE_NAMES.contains(*c) && !c.is_control()).collect();
    cleaned.trim().trim_end_matches('.').trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempFolder(PathBuf);

    impl TempFolder {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("trollboard_{name}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for TempFolder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn ramp() -> Vec<f32> {
        (0..4800).map(|i| i as f32 / 4800.0 - 0.5).collect()
    }

    fn names(library: &Library) -> Vec<String> {
        library.clips().into_iter().map(|clip| clip.name).collect()
    }

    #[test]
    fn recordings_are_numbered_and_round_trip() {
        let folder = TempFolder::new("numbered");
        let library = Library::open(folder.0.clone());

        let first = library.save_recording(&ramp(), 48_000).unwrap();
        let second = library.save_recording(&ramp(), 48_000).unwrap();
        assert_eq!(names(&library), ["clip_001", "clip_002"]);
        assert!((first.seconds - 0.1).abs() < 1e-6);
        assert_eq!(second.file_name, "clip_002.wav");

        let sound = read_mono(&library.path_of(&first)).unwrap();
        assert_eq!((sound.samples.len(), sound.sample_rate), (4800, 48_000));
        assert!(sound.samples.iter().zip(ramp()).all(|(read, written)| (read - written).abs() < 1e-3));
    }

    #[test]
    fn rename_cleans_the_name_and_refuses_duplicates() {
        let folder = TempFolder::new("rename");
        let library = Library::open(folder.0.clone());
        library.save_recording(&ramp(), 48_000).unwrap();
        library.save_recording(&ramp(), 48_000).unwrap();

        library.rename(0, "  bruh: moment?  ").unwrap();
        assert_eq!(names(&library), ["bruh moment", "clip_002"]);
        assert!(library.rename(1, "bruh moment").is_err());
        library.rename(0, "Bruh Moment").unwrap();
        assert_eq!(names(&library), ["Bruh Moment", "clip_002"]);
    }

    #[test]
    fn order_survives_a_reload_and_new_files_go_last() {
        let folder = TempFolder::new("order");
        let library = Library::open(folder.0.clone());
        library.save_recording(&ramp(), 48_000).unwrap();
        library.save_recording(&ramp(), 48_000).unwrap();
        library.move_clip(1, 0).unwrap();

        wav::write_mono(&folder.0.join("aaa.wav"), &ramp(), 48_000).unwrap();
        assert_eq!(names(&Library::open(folder.0.clone())), ["clip_002", "clip_001", "aaa"]);
    }

    #[test]
    fn import_copies_wav_files_only() {
        let folder = TempFolder::new("import");
        let outside = TempFolder::new("import_source");
        fs::create_dir_all(&outside.0).unwrap();
        let source = outside.0.join("air horn.wav");
        wav::write_mono(&source, &ramp(), 48_000).unwrap();

        let library = Library::open(folder.0.clone());
        assert_eq!(library.import(&source).unwrap().name, "air horn");
        assert_eq!(library.import(&source).unwrap().name, "air horn (2)");
        assert!(library.import(Path::new("Cargo.toml")).is_err());
    }
}
