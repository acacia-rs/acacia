//! The Java exporters (`exporter/*.java`, MIT): what the game builds in code rather than ships as
//! files (entity model layers, resolved item models), written out by running the client jar the
//! user downloaded with a JDK 25. Their output stays on that machine (docs/java-look.md
//! "Licensing"). Compiled once per source text, each run once per source text, under
//! `<java dir>/exporters`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::download::{Error, get, pinned};
use crate::jar::{VERSION, version_package};

const SOURCES: [(&str, &str); 6] = [
    ("BlockShapeExporter.java", include_str!("../exporter/BlockShapeExporter.java")),
    ("EntityModelExporter.java", include_str!("../exporter/EntityModelExporter.java")),
    ("AnimationExporter.java", include_str!("../exporter/AnimationExporter.java")),
    ("ItemModelExporter.java", include_str!("../exporter/ItemModelExporter.java")),
    ("ItemGeometry.java", include_str!("../exporter/ItemGeometry.java")),
    ("UnitSprites.java", include_str!("../exporter/UnitSprites.java")),
];

/// The version package's native classifier for this machine, after `natives-`.
const NATIVES: &str = match (std::env::consts::OS.as_bytes(), std::env::consts::ARCH.as_bytes()) {
    (b"windows", b"aarch64") => "windows-arm64",
    (b"windows", _) => "windows",
    (b"macos", b"aarch64") => "macos-arm64",
    (b"macos", _) => "macos",
    _ => "linux",
};

pub struct Exporter {
    jdk: PathBuf,
    classpath: String,
    work: PathBuf,
}

impl Exporter {
    /// Compiles the exporters unless the same sources already are; `None` without a JDK 25
    /// (JAVA_HOME or PATH).
    pub fn prepare(java_dir: &Path) -> Result<Option<Exporter>, Error> {
        let Some(jdk) = jdk() else { return Ok(None) };
        // The exporters run in `work`, so a relative classpath would miss.
        let java_dir = &std::path::absolute(java_dir)?;
        let separator = if cfg!(windows) { ";" } else { ":" };
        let work = java_dir.join("exporters");
        let mut classpath = vec![java_dir.join(format!("client-{VERSION}.jar"))];
        classpath.extend(libraries(&java_dir.join("libraries"))?);
        let classpath = classpath.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(separator);
        let stamp = work.join("sources.txt");
        let sources: String = SOURCES.iter().map(|(_, text)| *text).collect();
        if std::fs::read_to_string(&stamp).ok().as_deref() != Some(sources.as_str()) {
            let _ = std::fs::remove_dir_all(&work);
            let src = work.join("src");
            std::fs::create_dir_all(&src)?;
            for (name, text) in SOURCES {
                std::fs::write(src.join(name), text)?;
            }
            let mut javac = Command::new(jdk.join("javac"));
            javac.args(["--release", "25", "-proc:none", "-nowarn", "-cp", &classpath, "-d"]).arg(work.join("classes"));
            run(javac.args(SOURCES.iter().map(|(name, _)| src.join(name))))?;
            std::fs::write(&stamp, sources)?;
        }
        let classpath = format!("{}{separator}{classpath}", work.join("classes").display());
        Ok(Some(Exporter { jdk, classpath, work }))
    }

    /// `acacia.lookbake.<main>`'s output, run unless this build of the exporters already wrote it.
    pub fn output(&self, main: &str) -> Result<PathBuf, Error> {
        let out = self.work.join(format!("{main}-{VERSION}.json"));
        if !out.is_file() {
            let partial = out.with_extension("partial");
            // The game's bootstrap writes logs into the working directory.
            run(Command::new(self.jdk.join("java")).current_dir(&self.work).args(["-Xmx3G", "-Djava.awt.headless=true", "-cp", &self.classpath])
                .arg(format!("acacia.lookbake.{main}")).arg(&partial))?;
            std::fs::rename(partial, &out)?;
        }
        Ok(out)
    }
}

fn run(command: &mut Command) -> Result<(), Error> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("{command:?}: {status}").into());
    }
    Ok(())
}

/// The client's libraries from the version package, downloaded and checked into `dir`, with this
/// machine's native archives: `ClientBootstrap` loads LWJGL, which unpacks them from the classpath.
fn libraries(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let package = version_package()?;
    let mut out = Vec::new();
    for library in package["libraries"].as_array().into_iter().flatten() {
        let artifact = &library["downloads"]["artifact"];
        let (Some(path), Some(sha1), Some(url)) = (artifact["path"].as_str(), artifact["sha1"].as_str(), artifact["url"].as_str()) else { continue };
        if library["name"].as_str().and_then(|n| n.split_once(":natives-")).is_some_and(|(_, classifier)| classifier != NATIVES) {
            continue;
        }
        let file = dir.join(path);
        pinned(&file, sha1, || get(url))?;
        if !out.contains(&file) {
            out.push(file);
        }
    }
    Ok(out)
}

/// `bin` of `$JAVA_HOME`, else of the `javac` on the path, when it is release 25 or newer.
fn jdk() -> Option<PathBuf> {
    let home = std::env::var_os("JAVA_HOME").map(|h| PathBuf::from(h).join("bin"));
    let candidates = home.into_iter().chain([PathBuf::new()]);
    candidates.into_iter().find(|bin| {
        let output = Command::new(bin.join("javac")).arg("-version").output().ok();
        let text = output.map(|o| String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)).unwrap_or_default();
        release(&text).is_some_and(|r| r >= 25)
    })
}

/// `javac 25.0.1` to 25.
fn release(version: &str) -> Option<u32> {
    version.split_whitespace().nth(1)?.split(['.', '-', '+']).next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_javac_release() {
        assert_eq!(release("javac 25.0.1\n"), Some(25));
        assert_eq!(release("javac 21"), Some(21));
        assert_eq!(release("javac 26-ea"), Some(26));
        assert_eq!(release(""), None);
    }
}
