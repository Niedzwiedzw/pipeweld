use eyre::{eyre, Result, WrapErr};
use gtk::{prelude::*, Orientation};
use gtk::{Application, ApplicationWindow, Button};
use leptos::*;
use tracing::{info, instrument};
pub mod extensions {
    use super::*;
    pub struct Reactive<T> {
        inner: T,
        cx: Scope,
    }

    pub trait InScope: Sized {
        fn in_scope(cx: Scope) -> Reactive<Self>;
    }

    impl<T: Clone + 'static> Reactive<T> {
        pub fn reactive<F: Fn(&mut T) + 'static>(self, modifier: F) -> Self {
            let inner = self.inner.clone();
            create_effect(self.cx, move |_| {
                let mut inner = inner.clone();
                modifier(&mut inner);
            });
            self
        }
        pub fn constant<F: Fn(&mut T) + 'static>(mut self, modifier: F) -> Self {
            modifier(&mut self.inner);
            self
        }
    }
    impl<I> AsRef<I> for Reactive<I> {
        fn as_ref(&self) -> &I {
            &self.inner
        }
    }

    macro_rules! in_scope {
        ($ty:ty) => {
            impl InScope for $ty {
                fn in_scope(cx: Scope) -> Reactive<Self> {
                    Reactive {
                        inner: Self::builder().build(),
                        cx,
                    }
                }
            }
        };
    }

    impl Reactive<ApplicationWindow> {
        pub fn in_scope(cx: Scope, application: &Application) -> Self {
            Self {
                inner: ApplicationWindow::builder()
                    .application(application)
                    .build(),
                cx,
            }
        }
    }

    in_scope!(Button);
    in_scope!(gtk::Box);
}
use extensions::*;
use tracing_subscriber::EnvFilter;

fn app_id() -> String {
    format!("it.niedzwiedz.{}", clap::crate_name!())
}

pub struct AudioControls;

#[derive(Debug, Clone, Copy)]
pub struct DiffValue(i32);

impl std::fmt::Display for DiffValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(diff) = self;
        diff.gt(&0)
            .then(|| write!(f, "+{diff}%"))
            .unwrap_or_else(|| write!(f, "{diff}%"))
    }
}
impl AudioControls {
    #[instrument(ret, err)]
    pub fn change_volume_percent(diff: DiffValue) -> Result<()> {
        std::process::Command::new("pactl")
            .arg("set-sink-volume")
            .arg("@DEFAULT_SINK@")
            .arg(format!("{diff}"))
            .output()
            .wrap_err("running the command")
            .and_then(|out| {
                out.status
                    .success()
                    .then_some(())
                    .ok_or_else(|| eyre!("command failed"))
            })
    }
}

fn setup_tracing_subscriber() -> Result<()> {
    // Check if the RUST_LOG environment variable is set.
    // If it's set, use its value as the filter.
    // If it's not set, set the default filter to "info".

    // Parse the RUST_LOG value into an `EnvFilter`.
    let env_filter = std::env::var("RUST_LOG")
        .wrap_err("no RUST_LOG env present")
        .and_then(|rust_log| {
            EnvFilter::try_new(&rust_log).wrap_err_with(|| format!("parsing RUST_LOG ({rust_log})"))
        })
        .or_else(|_| EnvFilter::try_new("pipeweld=info,warn"))?;

    // Set up the tracing subscriber with the composed filter and pretty-printing of spans.
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .finish();

    // Set the global default tracing subscriber.
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber.");
    Ok(())
}

// Basic GTK app setup from https://gtk-rs.org/gtk4-rs/stable/latest/book/hello_world.html
fn main() {
    {
        if let Err(message) = setup_tracing_subscriber() {
            eprintln!("[ERROR] Setting up logging: {message}");
        }
    }
    _ = create_scope(create_runtime(), |cx| {
        // Create a new application
        let app = Application::builder().application_id(app_id()).build();

        // Connect to "activate" signal of `app`
        app.connect_activate(move |app| build_ui(cx, app));

        // Run the application
        app.run();
    });
}

fn build_ui(cx: Scope, app: &Application) {
    let diff_volume_button = move |diff: DiffValue| {
        Button::in_scope(cx).constant(move |btn| {
            btn.set_margin_top(12);
            btn.set_margin_bottom(12);
            btn.set_margin_start(12);
            btn.set_margin_end(12);
            btn.connect_clicked(move |_| {
                AudioControls::change_volume_percent(diff).ok();
            });
            btn.set_label(&format!("{diff}"));
        })
    };

    let window = Reactive::<ApplicationWindow>::in_scope(cx, app).constant(move |window| {
        window.set_child(Some(
            gtk::Box::in_scope(cx)
                .constant(move |gtk_box| {
                    gtk_box.set_orientation(Orientation::Vertical);
                    gtk_box.append(diff_volume_button(DiffValue(-5)).as_ref());
                    gtk_box.append(diff_volume_button(DiffValue(5)).as_ref());
                })
                .as_ref(),
        ))
    });

    // Present window
    window.as_ref().present();
}
