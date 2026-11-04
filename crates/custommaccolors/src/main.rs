//! `custommaccolors`: installs the root daemon that gives every app the custom accent color as it
//! starts, and is that daemon.

use std::ffi::c_int;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use eris_inject::daemon::{self, Client};
use eris_inject::procs::{self, App, Kind};
use eris_inject::inject::{self, Payload};
use eris_inject::launchd::Service;
use eris_inject::notify::Channel;

mod config;
mod icons;
mod window;

const PAYLOAD: &str = "libcustommaccolors.dylib";
/// The settings window, as an app: built next to `custommaccolors`, installed in /Applications.
const APP: &str = "CustomMacColors.app";
const INSTALLED_APP: &str = "/Applications/CustomMacColors.app";
/// The name the payload gives `payload!`.
const NAME: &str = "custommaccolors";

const SERVICE: Service = Service {
    label: "io.github.eriskii.custommaccolors",
    dir: "/Library/Application Support/CustomMacColors",
    log: "/Library/Logs/custommaccolors.log",
    files: &[("custommaccolors", 0o755), (inject::THREAD_HELPER, 0o755), (PAYLOAD, 0o644)],
    arguments: &["daemon"],
    link: Some("/usr/local/bin/custommaccolors"),
};

#[derive(Parser)]
#[command(version, about = "Any color as the macOS accent color and icon tint, in every app")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Install the daemon that loads into every app (run with sudo from the build directory).
    Install,
    /// Remove the daemon and its files; running apps keep the color until they quit.
    Uninstall,
    /// Show whether the daemon is running, and the settings.
    Status,
    /// Open the settings window.
    Settings {
        /// Draw the window into this PNG instead of showing it (for checking its layout).
        #[arg(long, hide = true)]
        snapshot: Option<PathBuf>,
    },
    /// Change a setting in every app at once: `set accent #FF6B00` (or `system`), `set icon-tint
    /// #00A0A0` (or `accent`).
    Set { key: String, value: String },
    /// Apply the settings file after editing it by hand.
    Reload,
    /// List the running processes the daemon loads into (or would).
    Targets,
    /// Load into one running app right now (needs root).
    Inject {
        pid: i32,
        #[arg(long)]
        payload: Option<PathBuf>,
    },
    #[command(hide = true)]
    Daemon,
    /// Map the payload into this process and start it, without root.
    #[command(hide = true)]
    SelfTest { payload: PathBuf },
}

fn payload(path: &Path) -> Payload<'_> {
    Payload { path, name: NAME, patches: &[] }
}

/// Never loaded into: the login window and the password prompt hold secrets, and the Dock is
/// yabai's to load into.
const SKIPPED: &[&str] = &["com.apple.loginwindow", "com.apple.SecurityAgent", "com.apple.dock"];

/// Everything that can show AppKit controls: regular apps, agents (menu bar extras, login items),
/// system UI, app extensions, and the XPC services that run AppKit's app loop (open and save
/// panels, System Settings panes). Not background-only processes, nor the helper apps browsers and
/// Electron keep in their Frameworks folders. In a process that never makes an NSApplication, the
/// payload does nothing.
fn shows_controls(app: &App) -> bool {
    match app.kind {
        Kind::Background => false,
        Kind::Service => true,
        Kind::Regular | Kind::Agent | Kind::Extension => {
            !(app.nested && app.in_frameworks && app.kind != Kind::Extension) && !SKIPPED.contains(&app.id.as_str())
        }
    }
}

struct Colors {
    payload: PathBuf,
    /// Held for the daemon's lifetime, so the published settings outlive whatever published them.
    settings: Option<Channel>,
}

impl Client for Colors {
    fn user_changed(&mut self, _uid: u32, home: &Path) {
        if self.settings.is_none() {
            self.settings = config::channel().map_err(|e| eris_inject::log!("{e}")).ok();
        }
        // Someone just logged in (or the daemon just started): their file is what every app gets.
        let path = config::path_in(home);
        match config::load(&path).and_then(|settings| config::publish(&settings)) {
            Ok(()) => {}
            Err(e) => eris_inject::log!("settings: {e}; using the defaults"),
        }
    }

    fn wants(&mut self, app: &App, _home: &Path) -> bool {
        shows_controls(app)
    }

    fn load(&mut self, pid: i32, _bundle_id: &str) -> Result<(), inject::Error> {
        inject::inject(pid, &payload(&self.payload))
    }
}

unsafe extern "C" {
    fn dlopen(path: *const std::ffi::c_char, mode: c_int) -> *mut std::ffi::c_void;
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Install => {
            SERVICE.install()?;
            install_app()?;
            // As the user, whose file it is: makes it, and sends it to the apps.
            as_console_user(&[&SERVICE.file("custommaccolors").to_string_lossy(), "reload"]);
            println!("installed; the daemon is loading into your apps as they start");
            println!("settings: CustomMacColors in Applications, or `custommaccolors set`");
            println!("apps already running get the color once they're relaunched, or with `sudo custommaccolors inject <pid>`");
            Ok(())
        }
        Command::Uninstall => {
            SERVICE.uninstall()?;
            if Path::new(INSTALLED_APP).exists() {
                std::fs::remove_dir_all(INSTALLED_APP).map_err(|e| format!("can't remove {INSTALLED_APP}: {e}"))?;
            }
            println!("uninstalled; apps go back to normal as they're relaunched");
            Ok(())
        }
        Command::Status => {
            match (SERVICE.installed(), SERVICE.pid()) {
                (_, Some(pid)) => println!("daemon: running (pid {pid})"),
                (true, None) => println!("daemon: installed but not running; see {}", SERVICE.log),
                (false, None) => println!("daemon: not installed (sudo custommaccolors install)"),
            }
            let path = config::path()?;
            let settings = config::load(&path)?;
            println!("settings file:  {}", path.display());
            print_settings(&settings);
            if config::live().is_some_and(|live| live != settings) {
                println!("(apps have different settings; `custommaccolors reload` sends them the file's)");
            }
            Ok(())
        }
        Command::Settings { snapshot } => window::run(snapshot),
        Command::Set { key, value } => {
            let path = config::path()?;
            let mut settings = config::load(&path)?;
            config::set(&mut settings, &key, &value)?;
            config::save(&path, &settings)?;
            print_settings(&settings);
            Ok(())
        }
        Command::Reload => {
            let path = config::path()?;
            let settings = config::load(&path)?;
            config::ensure(&path, &settings)?;
            config::publish(&settings)?;
            print_settings(&settings);
            Ok(())
        }
        Command::Targets => {
            let me = unsafe { libc::getuid() };
            let mut rows: Vec<(String, i32, &str)> = Vec::new();
            for pid in procs::all_pids() {
                let Some(app) = procs::owner(pid).filter(|&uid| uid == me).and(procs::executable(pid)).and_then(|exe| procs::app(&exe))
                else {
                    continue;
                };
                let verdict = if shows_controls(&app) { "yes" } else { "no" };
                rows.push((app.id, pid, verdict));
            }
            rows.sort();
            for (id, pid, verdict) in rows {
                println!("{verdict:4}{pid:>7}  {id}");
            }
            Ok(())
        }
        Command::Inject { pid, payload: path } => {
            let path = path.unwrap_or_else(|| SERVICE.file(PAYLOAD));
            inject::inject(pid, &payload(&path)).map_err(|e| format!("loading into {pid} failed: {e}"))
        }
        Command::Daemon => {
            eris_inject::log!("started; payload {}", SERVICE.file(PAYLOAD).display());
            daemon::run(&mut Colors { payload: SERVICE.file(PAYLOAD), settings: None })
        }
        Command::SelfTest { payload: path } => {
            // With AppKit here the payload runs for real, as it would in an app.
            const RTLD_NOW: c_int = 2;
            let appkit = c"/System/Library/Frameworks/AppKit.framework/AppKit";
            if unsafe { dlopen(appkit.as_ptr(), RTLD_NOW) }.is_null() {
                return Err("can't load AppKit".into());
            }
            inject::self_test(&payload(&path)).map_err(|e| format!("self-test failed: {e}"))?;
            println!("mapped and started; see `log show --last 1m --predicate 'eventMessage BEGINSWITH \"[CustomMacColors]\"'`");
            Ok(())
        }
    }
}

fn print_settings(s: &colors_core::settings::Settings) {
    println!("accent:         {}", s.accent.map_or_else(|| "system (System Settings' own)".to_string(), config::hex));
    println!("icon tint:      {}", s.icon_tint.map_or_else(|| "the accent".to_string(), config::hex));
}

/// Runs a command as whoever is at the screen, in their session, from root.
fn as_console_user(command: &[&str]) {
    let Some((uid, _)) = procs::console_user() else { return };
    let uid = uid.to_string();
    let user = format!("#{uid}");
    let mut args = vec!["asuser", uid.as_str(), "/usr/bin/sudo", "-H", "-u", user.as_str()];
    args.extend_from_slice(command);
    let _ = std::process::Command::new("/bin/launchctl").args(args).output();
}

/// Puts the settings app built next to this binary in /Applications.
fn install_app() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let built = exe.with_file_name(APP);
    if !built.exists() {
        return Err(format!("no {} next to {}; run make first", APP, exe.display()));
    }
    if Path::new(INSTALLED_APP).exists() {
        std::fs::remove_dir_all(INSTALLED_APP).map_err(|e| format!("can't replace {INSTALLED_APP}: {e}"))?;
    }
    let status = std::process::Command::new("/bin/cp").args(["-R"]).arg(&built).arg(INSTALLED_APP).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(format!("can't copy {} to {INSTALLED_APP}", built.display())),
    }
}

/// Opened as the app (from /Applications, Spotlight, …) rather than from a shell.
fn in_app_bundle() -> bool {
    std::env::current_exe().is_ok_and(|exe| exe.to_string_lossy().contains(".app/Contents/MacOS/"))
}

fn main() -> ExitCode {
    let command = match Cli::parse().command {
        Some(command) => command,
        None if in_app_bundle() => Command::Settings { snapshot: None },
        None => {
            let _ = Cli::command().print_help();
            return ExitCode::SUCCESS;
        }
    };
    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("custommaccolors: {e}");
            ExitCode::FAILURE
        }
    }
}
