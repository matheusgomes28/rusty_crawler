use std::borrow::Cow;

pub struct ProgressBar {
    bar: indicatif::ProgressBar,
}

impl ProgressBar {
    pub fn new(total_steps: u64) -> Self {
        let bar = indicatif::ProgressBar::new(total_steps);

        bar.set_style(
            indicatif::ProgressStyle::with_template(
                "{msg}\n[{elapsed}] {bar:40.white} {pos:>7}/{len:7}",
            )
            .unwrap(),
        );

        ProgressBar { bar }
    }

    pub fn set_step(&self, step: u64) {
        self.bar.set_position(step);
    }

    pub fn message(&self, msg: impl Into<Cow<'static, str>>) {
        self.bar.set_message(msg)
    }
}
