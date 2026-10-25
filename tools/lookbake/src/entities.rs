//! Java's entity models, which the game builds in code: `exporter/EntityModelExporter.java` (MIT)
//! runs against the user's client jar with a JDK 25 and writes every baked model layer, which goes
//! into the look as `java/entity_models.json` beside the jar's entity textures
//! (`java/textures/entity`). Without a JDK the look keeps Bedrock's entity geometry.
//! docs/java-look.md "Licensing" says why the tables are taken this way.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::download::{Error, get, pinned};
use crate::jar::{VERSION, version_package};
use crate::ui::copy_tree;

const SOURCE: &str = include_str!("../exporter/EntityModelExporter.java");
const MAIN: &str = "acacia.lookbake.EntityModelExporter";
pub const MODELS: &str = "java/entity_models.json";
const TEXTURES: &str = "textures/entity";

/// Exports the models (once per exporter source) and copies them and the textures into `out`.
/// `None` when no JDK 25 was found.
pub fn bake(java_dir: &Path, assets: &Path, out: &Path) -> Result<Option<usize>, Error> {
    let Some(jdk) = jdk() else {
        println!("  entities: no JDK 25 (JAVA_HOME or PATH), Bedrock geometry kept");
        return Ok(None);
    };
    let work = java_dir.join("entity-models");
    let models = work.join(format!("{VERSION}.json"));
    let stamp = work.join("exporter.java");
    if !models.is_file() || std::fs::read_to_string(&stamp).ok().as_deref() != Some(SOURCE) {
        export(&jdk, java_dir, &work, &models)?;
        std::fs::write(&stamp, SOURCE)?;
    }
    let layers = serde_json::from_slice::<serde_json::Map<String, Value>>(&std::fs::read(&models)?)?.len();
    let to = out.join(MODELS);
    std::fs::create_dir_all(to.parent().expect("a file in a directory"))?;
    std::fs::copy(&models, to)?;
    copy_tree(&assets.join(TEXTURES), &out.join("java").join(TEXTURES))?;
    Ok(Some(layers))
}

fn export(jdk: &Path, java_dir: &Path, work: &Path, models: &Path) -> Result<(), Error> {
    let separator = if cfg!(windows) { ";" } else { ":" };
    let mut classpath = vec![java_dir.join(format!("client-{VERSION}.jar"))];
    classpath.extend(libraries(&java_dir.join("libraries"))?);
    let classpath = classpath.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(separator);
    let (source, classes) = (work.join("src/EntityModelExporter.java"), work.join("classes"));
    std::fs::create_dir_all(source.parent().expect("a file in a directory"))?;
    std::fs::write(&source, SOURCE)?;
    run(Command::new(jdk.join("javac")).args(["--release", "25", "-proc:none", "-cp", &classpath, "-d"]).arg(&classes).arg(&source))?;
    let full = format!("{}{separator}{classpath}", classes.display());
    // The game's bootstrap writes logs into the working directory.
    run(Command::new(jdk.join("java")).current_dir(work).args(["-Xmx1G", "-cp", &full, MAIN]).arg(models))
}

fn run(command: &mut Command) -> Result<(), Error> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("{command:?}: {status}").into());
    }
    Ok(())
}

/// The client's libraries from the version package, downloaded and checked into `dir`. Native
/// archives are left out: nothing the exporter runs calls into them.
fn libraries(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let package = version_package()?;
    let mut out = Vec::new();
    for library in package["libraries"].as_array().into_iter().flatten() {
        let artifact = &library["downloads"]["artifact"];
        let (Some(path), Some(sha1), Some(url)) = (artifact["path"].as_str(), artifact["sha1"].as_str(), artifact["url"].as_str()) else { continue };
        if library["name"].as_str().is_some_and(|n| n.contains(":natives-")) {
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
