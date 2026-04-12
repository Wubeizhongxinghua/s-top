use ratatui::style::{Color, Style};

use crate::cli::ThemeChoice;

#[derive(Debug, Clone)]
pub struct Theme {
    pub accent: Style,
    pub mine: Style,
    pub other: Style,
    pub running: Style,
    pub pending: Style,
    pub running_pending_overlap: Style,
    pub warning: Style,
    pub danger: Style,
    pub success: Style,
    pub muted: Style,
    pub highlight: Style,
    pub title: Style,
    partition_palette: Vec<Color>,
}

impl Theme {
    pub fn from_choice(choice: ThemeChoice, no_color: bool) -> Self {
        if no_color {
            return Self {
                accent: Style::default(),
                mine: Style::default(),
                other: Style::default(),
                running: Style::default(),
                pending: Style::default(),
                running_pending_overlap: Style::default(),
                warning: Style::default(),
                danger: Style::default(),
                success: Style::default(),
                muted: Style::default(),
                highlight: Style::default(),
                title: Style::default(),
                partition_palette: vec![Color::Reset],
            };
        }

        match choice {
            ThemeChoice::Auto | ThemeChoice::Dark => Self {
                accent: Style::default().fg(Color::Cyan),
                mine: Style::default().fg(Color::LightBlue),
                other: Style::default().fg(Color::Yellow),
                running: Style::default().fg(Color::LightGreen),
                pending: Style::default().fg(Color::LightMagenta),
                running_pending_overlap: Style::default().fg(Color::LightRed),
                warning: Style::default().fg(Color::LightYellow),
                danger: Style::default().fg(Color::LightRed),
                success: Style::default().fg(Color::LightGreen),
                muted: Style::default().fg(Color::Gray),
                highlight: Style::default().bg(Color::Rgb(28, 54, 72)),
                title: Style::default().fg(Color::LightCyan),
                partition_palette: vec![
                    Color::LightBlue,
                    Color::LightCyan,
                    Color::LightGreen,
                    Color::LightMagenta,
                    Color::LightYellow,
                    Color::Yellow,
                    Color::Cyan,
                    Color::Green,
                    Color::Magenta,
                    Color::Blue,
                ],
            },
            ThemeChoice::Light => Self {
                accent: Style::default().fg(Color::Blue),
                mine: Style::default().fg(Color::Blue),
                other: Style::default().fg(Color::DarkGray),
                running: Style::default().fg(Color::Green),
                pending: Style::default().fg(Color::Magenta),
                running_pending_overlap: Style::default().fg(Color::Red),
                warning: Style::default().fg(Color::LightRed),
                danger: Style::default().fg(Color::Red),
                success: Style::default().fg(Color::Green),
                muted: Style::default().fg(Color::DarkGray),
                highlight: Style::default().bg(Color::Rgb(215, 229, 240)),
                title: Style::default().fg(Color::Blue),
                partition_palette: vec![
                    Color::Blue,
                    Color::Cyan,
                    Color::Green,
                    Color::Magenta,
                    Color::Yellow,
                    Color::LightBlue,
                    Color::LightCyan,
                    Color::LightGreen,
                    Color::LightMagenta,
                    Color::DarkGray,
                ],
            },
        }
    }

    pub fn partition_style(&self, partition: &str) -> Style {
        if self.partition_palette.len() == 1 && self.partition_palette[0] == Color::Reset {
            return Style::default();
        }
        let index = stable_partition_index(partition, self.partition_palette.len());
        Style::default().fg(self.partition_palette[index])
    }
}

fn stable_partition_index(partition: &str, len: usize) -> usize {
    let mut hash: u64 = 1469598103934665603;
    for byte in partition.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1099511628211);
    }
    (hash as usize) % len.max(1)
}

#[cfg(test)]
mod tests {
    use super::stable_partition_index;

    #[test]
    fn partition_color_mapping_is_stable() {
        let first = stable_partition_index("gpu_l48", 10);
        let second = stable_partition_index("gpu_l48", 10);
        let other = stable_partition_index("cpu_long", 10);

        assert_eq!(first, second);
        assert_ne!(first, other);
    }
}
