use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Typography {
    pub family: String,
    pub size_pt: f32,
    pub weight: u16,
    pub line_height: f32,
    pub paragraph_before: f32,
    pub paragraph_after: f32,
    pub indent: f32,
    pub justified: bool,
    pub width: f32,
    pub margins: [f32; 4],
    pub foreground: String,
    pub background: String,
    pub user_priority: bool,
    pub continuous: bool,
    pub show_progress: bool,
    pub show_toolbar: bool,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            family: "serif".into(),
            size_pt: 18.0,
            weight: 400,
            line_height: 1.55,
            paragraph_before: 0.0,
            paragraph_after: 0.6,
            indent: 1.5,
            justified: false,
            width: 720.0,
            margins: [28.0, 40.0, 28.0, 40.0],
            foreground: "#262A27".into(),
            background: "#F8F6F1".into(),
            user_priority: true,
            continuous: false,
            show_progress: true,
            show_toolbar: true,
        }
    }
}

impl Typography {
    pub fn validate(&mut self) {
        fn finite(value: f32, fallback: f32, low: f32, high: f32) -> f32 {
            if value.is_finite() {
                value.clamp(low, high)
            } else {
                fallback
            }
        }
        self.size_pt = finite(self.size_pt, 18.0, 12.0, 36.0);
        self.line_height = finite(self.line_height, 1.55, 1.15, 2.2);
        self.paragraph_before = finite(self.paragraph_before, 0.0, 0.0, 2.0);
        self.paragraph_after = finite(self.paragraph_after, 0.6, 0.0, 2.0);
        self.indent = finite(self.indent, 1.5, 0.0, 2.5);
        self.width = finite(self.width, 720.0, 240.0, 1600.0);
        for margin in &mut self.margins {
            *margin = finite(*margin, 28.0, 0.0, 120.0);
        }
        self.weight = match self.weight {
            0..=449 => 400,
            450..=599 => 500,
            _ => 700,
        };
        if !valid_color(&self.foreground) {
            self.foreground = "#262A27".into();
        }
        if !valid_color(&self.background) {
            self.background = "#F8F6F1".into();
        }
        self.family
            .retain(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'));
        self.family = self.family.chars().take(64).collect();
        if self.family.is_empty() {
            self.family = "serif".into();
        }
    }

    pub fn css(&self) -> String {
        let priority = if self.user_priority {
            " !important"
        } else {
            ""
        };
        let [top, right, bottom, left] = self.margins;
        // Set body font, not every descendant: headings/emphasis/code retain semantics.
        format!(
            "@page {{ margin: {top}pt {right}pt {bottom}pt {left}pt; }}\n\
            body {{ font-family: '{}'{priority}; font-size: {}pt{priority}; \
            line-height: {}{priority}; color: {}{priority}; background-color: {}{priority}; }}\n\
            p {{ line-height: {}{priority}; margin-top: {}em{priority}; margin-bottom: {}em{priority}; \
            text-indent: {}em{priority}; text-align: {}{priority}; }}\n\
            img, svg {{ max-width: 100%; height: auto; }}\n\
            pre {{ white-space: pre-wrap; }}\n",
            self.family,
            self.size_pt,
            self.line_height,
            self.foreground,
            self.background,
            self.line_height,
            self.paragraph_before,
            self.paragraph_after,
            self.indent,
            if self.justified { "justify" } else { "left" }
        )
    }
}

fn valid_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untrusted_settings_are_bounded_and_cannot_inject_css() {
        let mut s = Typography {
            family: "x';url(file:///secret);".into(),
            size_pt: f32::NAN,
            line_height: 50.0,
            background: "url(https://invalid)".into(),
            ..Typography::default()
        };
        s.validate();
        assert_eq!(s.size_pt, 18.0);
        assert_eq!(s.line_height, 2.2);
        assert!(!s.css().contains("file:"));
        assert!(!s.css().contains("https:"));
    }
}
