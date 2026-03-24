use egui::{Color32, Pos2, Rect, Response, Ui, Widget};

#[derive(Clone)]
pub struct HistoryBar {
    capacity: usize,
    values: Vec<Color32>,
}

impl HistoryBar {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity,
            values: Vec::<Color32>::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, color: Color32) {
        if self.values.len() == self.capacity {
            self.values.remove(0);
        }
        self.values.push(color);
    }
}

impl Widget for HistoryBar {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(self.capacity as f32, ui.available_height()), 
            egui::Sense::empty()
        );
        let x0 = rect.min.x + rect.width() - (self.values.len() as f32);
        let painter = ui.painter();
        for (i, &color) in self.values.iter().enumerate() {
            let x = x0 + i as f32;
            let strip_rect = Rect::from_min_max(
                Pos2::new(x, rect.min.y),
                Pos2::new(x + 1., rect.max.y),
            );
            painter.rect_filled(strip_rect, egui::CornerRadius::ZERO, color);
        }

        response
    }
}