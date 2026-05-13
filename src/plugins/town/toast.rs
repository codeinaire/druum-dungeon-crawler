//! Transient on-screen messages ("toasts") for Town actions.
//!
//! A simple queue-based notification system. Action handlers push messages
//! (e.g. "Aldric has been revived!") and the painter renders them as a stacked
//! overlay near the top-center of the screen. Each toast auto-dismisses after
//! [`DEFAULT_TOAST_TTL_SECS`] seconds, fading out as it expires.
//!
//! ## Scope
//!
//! Currently used by Town handlers (Temple, Inn, Shop, Guild). Promotable to a
//! general UI primitive if Combat needs it later — would move to
//! `plugins/ui/toast.rs` at that point.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

/// Default toast lifetime in seconds. Tuned for "long enough to read a short
/// sentence" without lingering long enough to clutter the screen.
pub const DEFAULT_TOAST_TTL_SECS: f32 = 3.0;

/// Cap on the queue so a pathological hot-key spam can't OOM the renderer.
pub const MAX_TOAST_QUEUE: usize = 8;

/// One on-screen message and its remaining lifetime.
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub remaining_secs: f32,
    pub initial_secs: f32,
}

/// Queue of active toasts, painted top-center.
#[derive(Resource, Default, Debug)]
pub struct Toasts {
    pub queue: Vec<Toast>,
}

impl Toasts {
    /// Push a toast with the default TTL ([`DEFAULT_TOAST_TTL_SECS`]).
    ///
    /// Older toasts are dropped if the queue exceeds [`MAX_TOAST_QUEUE`].
    pub fn push(&mut self, message: impl Into<String>) {
        self.push_with_ttl(message, DEFAULT_TOAST_TTL_SECS);
    }

    /// Push a toast with a custom TTL in seconds.
    pub fn push_with_ttl(&mut self, message: impl Into<String>, ttl_secs: f32) {
        self.queue.push(Toast {
            message: message.into(),
            remaining_secs: ttl_secs,
            initial_secs: ttl_secs,
        });
        // Drop oldest if over the cap.
        while self.queue.len() > MAX_TOAST_QUEUE {
            self.queue.remove(0);
        }
    }
}

/// Decrement TTLs and drop expired toasts. Runs in `Update`.
pub fn tick_toasts(time: Res<Time>, mut toasts: ResMut<Toasts>) {
    let dt = time.delta_secs();
    for t in &mut toasts.queue {
        t.remaining_secs -= dt;
    }
    toasts.queue.retain(|t| t.remaining_secs > 0.0);
}

/// Render active toasts as a top-center floating overlay. Each toast fades
/// alpha as it approaches expiry. Runs in `EguiPrimaryContextPass`.
pub fn paint_toasts(mut contexts: EguiContexts, toasts: Res<Toasts>) -> Result {
    if toasts.queue.is_empty() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    egui::Area::new(egui::Id::new("toasts_overlay"))
        .anchor(egui::Align2::CENTER_TOP, [0.0, 32.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            for toast in &toasts.queue {
                let life = (toast.remaining_secs / toast.initial_secs).clamp(0.0, 1.0);
                // Hold full opacity for the first 60% of life, then fade.
                let alpha = if life > 0.4 { 1.0 } else { life / 0.4 };
                let text_a = (alpha * 255.0) as u8;
                let bg_a = (alpha * 200.0) as u8;
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, bg_a))
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.colored_label(
                            egui::Color32::from_rgba_unmultiplied(255, 240, 200, text_a),
                            &toast.message,
                        );
                    });
                ui.add_space(4.0);
            }
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_records_message_with_default_ttl() {
        let mut t = Toasts::default();
        t.push("hello");
        assert_eq!(t.queue.len(), 1);
        assert_eq!(t.queue[0].message, "hello");
        assert_eq!(t.queue[0].remaining_secs, DEFAULT_TOAST_TTL_SECS);
        assert_eq!(t.queue[0].initial_secs, DEFAULT_TOAST_TTL_SECS);
    }

    #[test]
    fn push_with_ttl_overrides_default() {
        let mut t = Toasts::default();
        t.push_with_ttl("urgent", 1.5);
        assert_eq!(t.queue[0].remaining_secs, 1.5);
        assert_eq!(t.queue[0].initial_secs, 1.5);
    }

    #[test]
    fn queue_caps_at_max() {
        let mut t = Toasts::default();
        for i in 0..(MAX_TOAST_QUEUE + 3) {
            t.push(format!("msg {i}"));
        }
        assert_eq!(t.queue.len(), MAX_TOAST_QUEUE, "queue must cap at MAX_TOAST_QUEUE");
        // Oldest entries dropped; the last entries remain.
        let last_msg = format!("msg {}", MAX_TOAST_QUEUE + 2);
        assert_eq!(t.queue.last().unwrap().message, last_msg);
    }
}
