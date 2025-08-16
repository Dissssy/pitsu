use eframe::egui::{
    Color32, CornerRadius, NumExt, Pos2, Rect, Response, Rgba, Sense, Shape, Stroke, TextStyle, TextWrapMode, Ui, Vec2,
    Widget, WidgetInfo, WidgetText, WidgetType, lerp, vec2,
};

enum DoubleProgressBarText {
    Custom(WidgetText),
    Percentage,
}

/// A double progress bar, with two overlapped progress values.
/// The first (background) is blue, the second (foreground) is green.
#[must_use = "You should put this widget in a ui with `ui.add(widget);`"]
pub struct DoubleProgressBar {
    progress1: f32,
    progress2: f32,
    desired_width: Option<f32>,
    desired_height: Option<f32>,
    text: Option<DoubleProgressBarText>,
    animate: bool,
    corner_radius: Option<CornerRadius>,
}

impl DoubleProgressBar {
    /// Create a new double progress bar with two progress values in the `[0, 1]` range.
    pub fn new(progress1: f32, progress2: f32) -> Self {
        Self {
            progress1: progress1.clamp(0.0, 1.0),
            progress2: progress2.clamp(0.0, 1.0),
            desired_width: None,
            desired_height: None,
            text: None,
            animate: false,
            corner_radius: None,
        }
    }

    /// The desired width of the bar. Will use all horizontal space if not set.
    #[inline]
    pub fn desired_width(mut self, desired_width: f32) -> Self {
        self.desired_width = Some(desired_width);
        self
    }

    /// The desired height of the bar. Will use the default interaction size if not set.
    #[inline]
    pub fn desired_height(mut self, desired_height: f32) -> Self {
        self.desired_height = Some(desired_height);
        self
    }

    /// A custom text to display on the progress bar.
    #[inline]
    pub fn text(mut self, text: impl Into<WidgetText>) -> Self {
        self.text = Some(DoubleProgressBarText::Custom(text.into()));
        self
    }

    /// Show the progress in percent on the progress bar (shows progress2).
    #[inline]
    pub fn show_percentage(mut self) -> Self {
        self.text = Some(DoubleProgressBarText::Percentage);
        self
    }

    /// Whether to display a loading animation when progress2 `< 1`.
    #[inline]
    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }

    /// Set the rounding of the progress bar.
    #[inline]
    pub fn corner_radius(mut self, corner_radius: impl Into<CornerRadius>) -> Self {
        self.corner_radius = Some(corner_radius.into());
        self
    }

    #[inline]
    #[deprecated = "Renamed to `corner_radius`"]
    pub fn rounding(self, corner_radius: impl Into<CornerRadius>) -> Self {
        self.corner_radius(corner_radius)
    }
}

impl Widget for DoubleProgressBar {
    fn ui(self, ui: &mut Ui) -> Response {
        let Self {
            progress1,
            progress2,
            desired_width,
            desired_height,
            text,
            animate,
            corner_radius,
        } = self;

        let animate = animate && progress2 < 1.0;

        let desired_width = desired_width.unwrap_or_else(|| ui.available_size_before_wrap().x.at_least(96.0));
        let height = desired_height.unwrap_or(ui.spacing().interact_size.y);
        let (outer_rect, response) = ui.allocate_exact_size(vec2(desired_width, height), Sense::hover());

        response.widget_info(|| {
            let mut info = if let Some(DoubleProgressBarText::Custom(text)) = &text {
                WidgetInfo::labeled(WidgetType::ProgressIndicator, ui.is_enabled(), text.text())
            } else {
                WidgetInfo::new(WidgetType::ProgressIndicator)
            };
            info.value = Some((progress2 as f64 * 100.0).floor());
            info
        });

        if ui.is_rect_visible(response.rect) {
            if animate {
                ui.ctx().request_repaint();
            }

            let visuals = ui.style().visuals.clone();
            let has_custom_cr = corner_radius.is_some();
            let half_height = outer_rect.height() / 2.0;
            let corner_radius = corner_radius.unwrap_or_else(|| half_height.into());
            // Draw background
            ui.painter()
                .rect_filled(outer_rect, corner_radius, visuals.extreme_bg_color);

            // Draw first (bottom) progress bar in blue
            let min_width = 2.0 * f32::max(corner_radius.sw as _, corner_radius.nw as _).at_most(half_height);
            let filled_width1 = (outer_rect.width() * progress1).at_least(min_width);
            let inner_rect1 = Rect::from_min_size(outer_rect.min, vec2(filled_width1, outer_rect.height()));
            ui.painter()
                .rect_filled(inner_rect1, corner_radius, Color32::from_rgb(0, 120, 255));

            // Draw second (top) progress bar in green, overlapping
            let filled_width2 = (outer_rect.width() * progress2).at_least(min_width);
            let inner_rect2 = Rect::from_min_size(outer_rect.min, vec2(filled_width2, outer_rect.height()));
            let (dark, bright) = (0.7, 1.0);
            let color_factor = if animate {
                let time = ui.input(|i| i.time);
                lerp(dark..=bright, time.cos().abs())
            } else {
                bright
            };
            let green = Color32::from(Rgba::from(Color32::from_rgb(0, 200, 0)) * color_factor as f32);
            ui.painter().rect_filled(inner_rect2, corner_radius, green);

            // Optional: animation ring for progress2
            if animate && !has_custom_cr {
                let n_points = 20;
                let time = ui.input(|i| i.time);
                let start_angle = time * std::f64::consts::TAU;
                let end_angle = start_angle + 240f64.to_radians() * time.sin();
                let circle_radius = half_height - 2.0;
                let points: Vec<Pos2> = (0..n_points)
                    .map(|i| {
                        let angle = lerp(start_angle..=end_angle, i as f64 / n_points as f64);
                        let (sin, cos) = angle.sin_cos();
                        inner_rect2.right_center()
                            + circle_radius * vec2(cos as f32, sin as f32)
                            + vec2(-half_height, 0.0)
                    })
                    .collect();
                ui.painter()
                    .add(Shape::line(points, Stroke::new(2.0, visuals.text_color())));
            }

            if let Some(text_kind) = text {
                let text = match text_kind {
                    DoubleProgressBarText::Custom(text) => text,
                    DoubleProgressBarText::Percentage => format!("{}%", (progress2 * 100.0) as usize).into(),
                };
                let galley = text.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, TextStyle::Button);
                let text_pos = outer_rect.left_center() - Vec2::new(0.0, galley.size().y / 2.0)
                    + vec2(ui.spacing().item_spacing.x, 0.0);
                let text_color = visuals.override_text_color.unwrap_or(visuals.selection.stroke.color);
                ui.painter()
                    .with_clip_rect(outer_rect)
                    .galley(text_pos, galley, text_color);
            }
        }

        response
    }
}
