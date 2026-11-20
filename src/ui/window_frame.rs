use super::theme;
use eframe::egui::viewport::ResizeDirection;
use eframe::egui::{
    self, Color32, Context, CursorIcon, Id, LayerId, Order, PointerButton, Pos2, Rect, Response, Role, Sense, Stroke,
    StrokeKind, Ui, Vec2, ViewportCommand, WidgetInfo, emath::GuiRounding, pos2, vec2,
};

const BUTTON_SIZE: Vec2 = vec2(34.0, 28.0);
const BUTTON_GAP: f32 = 2.0;
pub const CONTROLS_SIZE: Vec2 = vec2(BUTTON_SIZE.x * 3.0 + BUTTON_GAP * 2.0, BUTTON_SIZE.y);

const RESIZE_BORDER: f32 = 5.0;
const RESIZE_CORNER: f32 = 14.0;
const ICON_HALF_SIZE: f32 = 5.0;

pub fn title_bar(ui: &mut Ui, rect: Rect) {
    let response = ui.interact(rect, Id::unique("title_bar"), Sense::click_and_drag());
    if response.double_clicked() {
        toggle_maximized(ui.ctx());
    } else if response.drag_started_by(PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

pub fn window_buttons(ctx: &Context, slot: Rect) {
    egui::Area::new(Id::unique("window_buttons")).order(Order::Tooltip).fixed_pos(slot.min).show(ctx, |ui| {
        ui.spacing_mut().item_spacing = vec2(BUTTON_GAP, 0.0);
        ui.horizontal(|ui| {
            if window_button(ui, Icon::Minimize, "Minimize").clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
            let (icon, name) =
                if is_maximized(ctx) { (Icon::Restore, "Restore") } else { (Icon::Maximize, "Maximize") };
            if window_button(ui, icon, name).clicked() {
                toggle_maximized(ctx);
            }
            if window_button(ui, Icon::Close, "Close").clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        });
    });
}

pub fn border_and_resize_handles(ctx: &Context) {
    if is_maximized(ctx) {
        return;
    }
    let window = ctx.content_rect();
    ctx.layer_painter(LayerId::new(Order::Tooltip, Id::unique("window_border"))).rect_stroke(
        window,
        0.0,
        Stroke::new(1.0, theme::SURFACE_RAISED),
        StrokeKind::Inside,
    );

    for (index, edge) in edges(window).into_iter().enumerate() {
        egui::Area::new(Id::unique(("resize_handle", index))).order(Order::Tooltip).fixed_pos(edge.min).show(
            ctx,
            |ui| {
                let (_, response) = ui.allocate_exact_size(edge.size(), Sense::drag());
                if let Some(pointer) = response.hover_pos() {
                    let (direction, cursor) = resize_direction(window, pointer);
                    ctx.set_cursor_icon(cursor);
                    if ctx.input(|input| input.pointer.primary_pressed()) {
                        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
                    }
                }
            },
        );
    }
}

fn is_maximized(ctx: &Context) -> bool {
    ctx.input(|input| input.viewport().maximized.unwrap_or(false))
}

fn toggle_maximized(ctx: &Context) {
    ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized(ctx)));
}

fn edges(window: Rect) -> [Rect; 4] {
    let inner_top = window.min.y + RESIZE_BORDER;
    let inner_bottom = window.max.y - RESIZE_BORDER;
    [
        Rect::from_min_max(window.min, pos2(window.max.x, inner_top)),
        Rect::from_min_max(pos2(window.min.x, inner_bottom), window.max),
        Rect::from_min_max(pos2(window.min.x, inner_top), pos2(window.min.x + RESIZE_BORDER, inner_bottom)),
        Rect::from_min_max(pos2(window.max.x - RESIZE_BORDER, inner_top), pos2(window.max.x, inner_bottom)),
    ]
}

fn resize_direction(window: Rect, pointer: Pos2) -> (ResizeDirection, CursorIcon) {
    let top = pointer.y < window.min.y + RESIZE_CORNER;
    let bottom = pointer.y > window.max.y - RESIZE_CORNER;
    let left = pointer.x < window.min.x + RESIZE_CORNER;
    let right = pointer.x > window.max.x - RESIZE_CORNER;
    match (top, bottom, left, right) {
        (true, _, true, _) => (ResizeDirection::NorthWest, CursorIcon::ResizeNwSe),
        (true, _, _, true) => (ResizeDirection::NorthEast, CursorIcon::ResizeNeSw),
        (_, true, true, _) => (ResizeDirection::SouthWest, CursorIcon::ResizeNeSw),
        (_, true, _, true) => (ResizeDirection::SouthEast, CursorIcon::ResizeNwSe),
        (true, ..) => (ResizeDirection::North, CursorIcon::ResizeVertical),
        (_, true, ..) => (ResizeDirection::South, CursorIcon::ResizeVertical),
        (_, _, true, _) => (ResizeDirection::West, CursorIcon::ResizeHorizontal),
        _ => (ResizeDirection::East, CursorIcon::ResizeHorizontal),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Icon {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn window_button(ui: &mut Ui, icon: Icon, name: &'static str) -> Response {
    let (rect, response) = ui.allocate_exact_size(BUTTON_SIZE, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name));

    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();
    let (fill, color) = match (icon == Icon::Close, pressed, hovered) {
        (true, true, _) => (theme::RED_PRESSED, Color32::WHITE),
        (true, false, true) => (theme::RED_STRONG, Color32::WHITE),
        (false, true, _) => (theme::SURFACE_PRESSED, Color32::WHITE),
        (false, false, true) => (theme::SURFACE_RAISED, theme::TEXT),
        _ => (Color32::TRANSPARENT, theme::TEXT_MUTED),
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    paint_icon(ui, icon, rect.center(), Stroke::new(1.0, color));
    response.on_hover_text(name)
}

fn paint_icon(ui: &Ui, icon: Icon, center: Pos2, stroke: Stroke) {
    let painter = ui.painter();
    let center = center.round_to_pixel_center(ui.ctx().pixels_per_point());
    let half = ICON_HALF_SIZE;
    match icon {
        Icon::Minimize => {
            painter.line_segment([center + vec2(-half, 0.0), center + vec2(half, 0.0)], stroke);
        }
        Icon::Maximize => {
            let square = Rect::from_center_size(center, Vec2::splat(half * 2.0));
            painter.rect_stroke(square, 1.5, stroke, StrokeKind::Middle);
        }
        Icon::Restore => {
            let front = Rect::from_min_max(center + vec2(-half, 2.0 - half), center + vec2(half - 2.0, half));
            painter.rect_stroke(front, 1.5, stroke, StrokeKind::Middle);
            let back_top_left = center + vec2(2.0 - half, -half);
            let back_top_right = center + vec2(half, -half);
            let back_bottom_right = center + vec2(half, half - 2.0);
            painter.line_segment([back_top_left, back_top_right], stroke);
            painter.line_segment([back_top_right, back_bottom_right], stroke);
        }
        Icon::Close => {
            painter.line_segment([center + vec2(-half, -half), center + vec2(half, half)], stroke);
            painter.line_segment([center + vec2(-half, half), center + vec2(half, -half)], stroke);
        }
    }
}
