//! Core gesture definitions, mathematical analysis, and state machine.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::f64::consts::PI;
use std::fmt;
use std::str::FromStr;
use std::time::{Duration, Instant};

/// The primary gesture classification types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GestureType {
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
    PinchIn,
    PinchOut,
    RotateCw,
    RotateCcw,
    EdgeSwipeLeft,
    EdgeSwipeRight,
    EdgeSwipeUp,
    EdgeSwipeDown,
    Tap,
    Hold,
}

impl fmt::Display for GestureType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GestureType::SwipeLeft => write!(f, "swipe_left"),
            GestureType::SwipeRight => write!(f, "swipe_right"),
            GestureType::SwipeUp => write!(f, "swipe_up"),
            GestureType::SwipeDown => write!(f, "swipe_down"),
            GestureType::PinchIn => write!(f, "pinch_in"),
            GestureType::PinchOut => write!(f, "pinch_out"),
            GestureType::RotateCw => write!(f, "rotate_cw"),
            GestureType::RotateCcw => write!(f, "rotate_ccw"),
            GestureType::EdgeSwipeLeft => write!(f, "edge_swipe_left"),
            GestureType::EdgeSwipeRight => write!(f, "edge_swipe_right"),
            GestureType::EdgeSwipeUp => write!(f, "edge_swipe_up"),
            GestureType::EdgeSwipeDown => write!(f, "edge_swipe_down"),
            GestureType::Tap => write!(f, "tap"),
            GestureType::Hold => write!(f, "hold"),
        }
    }
}

/// Unified cross-platform gesture event with finger count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Gesture {
    pub gesture_type: GestureType,
    pub fingers: usize,
}

impl Gesture {
    pub fn new(gesture_type: GestureType, fingers: usize) -> Self {
        Self { gesture_type, fingers }
    }
}

impl fmt::Display for Gesture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.gesture_type {
            GestureType::PinchIn if self.fingers == 2 => write!(f, "pinch_in"),
            GestureType::PinchOut if self.fingers == 2 => write!(f, "pinch_out"),
            GestureType::RotateCw if self.fingers == 2 => write!(f, "rotate_cw"),
            GestureType::RotateCcw if self.fingers == 2 => write!(f, "rotate_ccw"),
            GestureType::EdgeSwipeLeft => write!(f, "edge_swipe_left"),
            GestureType::EdgeSwipeRight => write!(f, "edge_swipe_right"),
            GestureType::EdgeSwipeUp => write!(f, "edge_swipe_up"),
            GestureType::EdgeSwipeDown => write!(f, "edge_swipe_down"),
            _ => write!(f, "{}_{}", self.gesture_type, self.fingers),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseGestureError(pub String);

impl fmt::Display for ParseGestureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to parse gesture: {}", self.0)
    }
}

impl std::error::Error for ParseGestureError {}

impl FromStr for Gesture {
    type Err = ParseGestureError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_lowercase();
        match normalized.as_str() {
            "pinch_in" => return Ok(Gesture::new(GestureType::PinchIn, 2)),
            "pinch_out" => return Ok(Gesture::new(GestureType::PinchOut, 2)),
            "rotate_cw" | "rotate_clockwise" => return Ok(Gesture::new(GestureType::RotateCw, 2)),
            "rotate_ccw" | "rotate_counter_clockwise" => {
                return Ok(Gesture::new(GestureType::RotateCcw, 2))
            }
            "edge_swipe_left" => return Ok(Gesture::new(GestureType::EdgeSwipeLeft, 1)),
            "edge_swipe_right" => return Ok(Gesture::new(GestureType::EdgeSwipeRight, 1)),
            "edge_swipe_up" => return Ok(Gesture::new(GestureType::EdgeSwipeUp, 1)),
            "edge_swipe_down" => return Ok(Gesture::new(GestureType::EdgeSwipeDown, 1)),
            _ => {}
        }

        let parts: Vec<&str> = normalized.rsplitn(2, '_').collect();
        if parts.len() != 2 {
            return Err(ParseGestureError(format!(
                "Invalid format '{}'. Expected format like 'swipe_left_3', 'pinch_in_3', or 'tap_4'",
                s
            )));
        }

        let fingers: usize = parts[0]
            .parse()
            .map_err(|_| ParseGestureError(format!("Invalid finger count in '{}'", s)))?;

        let g_type = match parts[1] {
            "swipe_left" => GestureType::SwipeLeft,
            "swipe_right" => GestureType::SwipeRight,
            "swipe_up" => GestureType::SwipeUp,
            "swipe_down" => GestureType::SwipeDown,
            "pinch_in" => GestureType::PinchIn,
            "pinch_out" => GestureType::PinchOut,
            "rotate_cw" => GestureType::RotateCw,
            "rotate_ccw" => GestureType::RotateCcw,
            "tap" => GestureType::Tap,
            "hold" => GestureType::Hold,
            _ => return Err(ParseGestureError(format!("Unknown gesture action in '{}'", s))),
        };

        Ok(Gesture::new(g_type, fingers))
    }
}

impl Serialize for Gesture {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Gesture {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Gesture::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// A single finger contact point on the touchpad surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    pub id: i32,
    pub x: f64,
    pub y: f64,
}

/// A snapshot of all active finger contacts at a specific instant.
#[derive(Debug, Clone)]
pub struct TouchFrame {
    pub timestamp: Instant,
    pub points: Vec<TouchPoint>,
}

impl TouchFrame {
    pub fn new(points: Vec<TouchPoint>) -> Self {
        Self {
            timestamp: Instant::now(),
            points,
        }
    }

    pub fn centroid(&self) -> Option<(f64, f64)> {
        if self.points.is_empty() {
            return None;
        }
        let (sum_x, sum_y) = self
            .points
            .iter()
            .fold((0.0, 0.0), |acc, p| (acc.0 + p.x, acc.1 + p.y));
        let count = self.points.len() as f64;
        Some((sum_x / count, sum_y / count))
    }

    pub fn spread(&self) -> Option<f64> {
        let (cx, cy) = self.centroid()?;
        if self.points.len() < 2 {
            return Some(0.0);
        }
        let total_dist: f64 = self
            .points
            .iter()
            .map(|p| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt())
            .sum();
        Some(total_dist / (self.points.len() as f64))
    }

    /// Calculate the relative angle between the first two contacts in radians.
    pub fn angle(&self) -> Option<f64> {
        if self.points.len() >= 2 {
            let p1 = self.points[0];
            let p2 = self.points[1];
            Some((p2.y - p1.y).atan2(p2.x - p1.x))
        } else {
            None
        }
    }
}

/// Dynamic sensitivity configuration for gesture recognition algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GestureSensitivity {
    /// Minimum centroid movement in coordinate units to trigger a swipe.
    pub swipe_threshold: f64,
    /// Minimum swipe velocity (units / millisecond) required.
    pub min_swipe_velocity: f64,
    /// Pinch distance delta ratio relative to initial spread (e.g., 0.20 = 20% change).
    pub pinch_threshold: f64,
    /// Rotation angle threshold in degrees for twist gestures.
    pub rotation_threshold_deg: f64,
    /// Maximum duration in milliseconds for a multi-touch tap.
    pub tap_max_duration_ms: u64,
    /// Minimum duration in milliseconds for a tap-and-hold gesture.
    pub hold_min_duration_ms: u64,
    /// Maximum contact displacement in units to qualify as a stationary tap or hold.
    pub tap_max_displacement: f64,
    /// Enable 2-finger swipe gestures (when disabled, 2-finger scrolling is left to OS).
    pub two_finger_swipes_enabled: bool,
    /// Invert horizontal swipe direction (natural vs traditional).
    pub invert_swipe_x: bool,
    /// Invert vertical swipe direction (natural vs traditional).
    pub invert_swipe_y: bool,
}

impl Default for GestureSensitivity {
    fn default() -> Self {
        Self {
            swipe_threshold: 45.0,
            min_swipe_velocity: 0.10,
            pinch_threshold: 0.20,
            rotation_threshold_deg: 24.0,
            tap_max_duration_ms: 280,
            hold_min_duration_ms: 650,
            tap_max_displacement: 18.0,
            two_finger_swipes_enabled: true,
            invert_swipe_x: false,
            invert_swipe_y: false,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum State {
    Idle,
    Tracking {
        start_time: Instant,
        initial_centroid: (f64, f64),
        initial_spread: f64,
        initial_angle: Option<f64>,
        finger_count: usize,
        max_displacement: f64,
    },
    Triggered {
        gesture: Gesture,
        registered_at: Instant,
    },
}

pub struct GestureRecognizer {
    state: State,
    pub sensitivity: GestureSensitivity,
}

impl GestureRecognizer {
    pub fn new(sensitivity: GestureSensitivity) -> Self {
        Self {
            state: State::Idle,
            sensitivity,
        }
    }

    pub fn process_frame(&mut self, frame: &TouchFrame) -> Option<Gesture> {
        let active_count = frame.points.len();

        match &self.state {
            State::Idle => {
                if active_count >= 1 {
                    if let (Some(centroid), Some(spread)) = (frame.centroid(), frame.spread()) {
                        self.state = State::Tracking {
                            start_time: frame.timestamp,
                            initial_centroid: centroid,
                            initial_spread: spread,
                            initial_angle: frame.angle(),
                            finger_count: active_count,
                            max_displacement: 0.0,
                        };
                    }
                }
                None
            }

            State::Tracking {
                start_time,
                initial_centroid,
                initial_spread,
                initial_angle,
                finger_count,
                max_displacement,
            } => {
                let start_time = *start_time;
                let (init_cx, init_cy) = *initial_centroid;
                let init_spread = *initial_spread;
                let init_angle_opt = *initial_angle;
                let fingers = *finger_count;
                let mut current_max_disp = *max_displacement;

                // When fingers are lifted:
                if active_count == 0 {
                    let duration = frame.timestamp.duration_since(start_time);
                    self.state = State::Idle;

                    // Tap recognition
                    if duration <= Duration::from_millis(self.sensitivity.tap_max_duration_ms)
                        && current_max_disp <= self.sensitivity.tap_max_displacement
                        && (2..=5).contains(&fingers)
                    {
                        return Some(Gesture::new(GestureType::Tap, fingers));
                    }

                    // Hold recognition
                    if duration >= Duration::from_millis(self.sensitivity.hold_min_duration_ms)
                        && current_max_disp <= self.sensitivity.tap_max_displacement
                        && (2..=5).contains(&fingers)
                    {
                        return Some(Gesture::new(GestureType::Hold, fingers));
                    }

                    return None;
                }

                if let (Some(cur_cx), Some(cur_cy)) = (frame.centroid().map(|c| c.0), frame.centroid().map(|c| c.1)) {
                    let mut dx = cur_cx - init_cx;
                    let mut dy = cur_cy - init_cy;

                    if self.sensitivity.invert_swipe_x {
                        dx = -dx;
                    }
                    if self.sensitivity.invert_swipe_y {
                        dy = -dy;
                    }

                    let displacement = (dx.powi(2) + dy.powi(2)).sqrt();
                    if displacement > current_max_disp {
                        current_max_disp = displacement;
                    }

                    let cur_spread = frame.spread().unwrap_or(init_spread);
                    let elapsed_ms = frame.timestamp.duration_since(start_time).as_millis().max(1) as f64;
                    let velocity = displacement / elapsed_ms;

                    // 1. Two-Finger Twist / Rotation recognition
                    if fingers == 2 && init_spread > 15.0 {
                        if let (Some(init_ang), Some(cur_ang)) = (init_angle_opt, frame.angle()) {
                            let mut delta_ang = cur_ang - init_ang;
                            while delta_ang > PI {
                                delta_ang -= 2.0 * PI;
                            }
                            while delta_ang < -PI {
                                delta_ang += 2.0 * PI;
                            }
                            let deg = delta_ang * 180.0 / PI;

                            if deg.abs() >= self.sensitivity.rotation_threshold_deg {
                                let g_type = if deg > 0.0 {
                                    GestureType::RotateCw
                                } else {
                                    GestureType::RotateCcw
                                };
                                let gesture = Gesture::new(g_type, 2);
                                self.state = State::Triggered {
                                    gesture,
                                    registered_at: frame.timestamp,
                                };
                                return Some(gesture);
                            }
                        }
                    }

                    // 2. Pinch recognition (2, 3, or 4 fingers)
                    if (2..=4).contains(&fingers) && init_spread > 12.0 {
                        let ratio = cur_spread / init_spread;
                        if ratio <= (1.0 - self.sensitivity.pinch_threshold) {
                            let gesture = Gesture::new(GestureType::PinchIn, fingers);
                            self.state = State::Triggered {
                                gesture,
                                registered_at: frame.timestamp,
                            };
                            return Some(gesture);
                        } else if ratio >= (1.0 + self.sensitivity.pinch_threshold) {
                            let gesture = Gesture::new(GestureType::PinchOut, fingers);
                            self.state = State::Triggered {
                                gesture,
                                registered_at: frame.timestamp,
                            };
                            return Some(gesture);
                        }
                    }

                    // 3. Multi-finger Swipes (2, 3, 4 fingers)
                    let allow_swipe = if fingers == 2 {
                        self.sensitivity.two_finger_swipes_enabled
                    } else {
                        fingers >= 3
                    };

                    if allow_swipe
                        && displacement >= self.sensitivity.swipe_threshold
                        && velocity >= self.sensitivity.min_swipe_velocity
                    {
                        let g_type = if dx.abs() > dy.abs() {
                            if dx > 0.0 {
                                GestureType::SwipeRight
                            } else {
                                GestureType::SwipeLeft
                            }
                        } else if dy > 0.0 {
                            GestureType::SwipeDown
                        } else {
                            GestureType::SwipeUp
                        };

                        let gesture = Gesture::new(g_type, fingers);
                        self.state = State::Triggered {
                            gesture,
                            registered_at: frame.timestamp,
                        };
                        return Some(gesture);
                    }

                    self.state = State::Tracking {
                        start_time,
                        initial_centroid: (init_cx, init_cy),
                        initial_spread: init_spread,
                        initial_angle: init_angle_opt,
                        finger_count: fingers.max(active_count),
                        max_displacement: current_max_disp,
                    };
                }
                None
            }

            State::Triggered { .. } => {
                if active_count == 0 {
                    self.state = State::Idle;
                }
                None
            }
        }
    }

    #[allow(dead_code)]
    pub fn process_delta_swipe(&mut self, fingers: usize, dx: f64, dy: f64) -> Option<Gesture> {
        let displacement = (dx.powi(2) + dy.powi(2)).sqrt();
        if displacement >= self.sensitivity.swipe_threshold {
            let g_type = if dx.abs() > dy.abs() {
                if dx > 0.0 {
                    GestureType::SwipeRight
                } else {
                    GestureType::SwipeLeft
                }
            } else if dy > 0.0 {
                GestureType::SwipeDown
            } else {
                GestureType::SwipeUp
            };
            Some(Gesture::new(g_type, fingers))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn process_delta_pinch(&mut self, fingers: usize, scale: f64) -> Option<Gesture> {
        if scale <= (1.0 - self.sensitivity.pinch_threshold) {
            Some(Gesture::new(GestureType::PinchIn, fingers))
        } else if scale >= (1.0 + self.sensitivity.pinch_threshold) {
            Some(Gesture::new(GestureType::PinchOut, fingers))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.state = State::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gesture_parsing_and_formatting() {
        let g = "swipe_left_3".parse::<Gesture>().unwrap();
        assert_eq!(g, Gesture::new(GestureType::SwipeLeft, 3));
        assert_eq!(g.to_string(), "swipe_left_3");

        let p_in = "pinch_in".parse::<Gesture>().unwrap();
        assert_eq!(p_in, Gesture::new(GestureType::PinchIn, 2));
        assert_eq!(p_in.to_string(), "pinch_in");

        let rot = "rotate_cw".parse::<Gesture>().unwrap();
        assert_eq!(rot, Gesture::new(GestureType::RotateCw, 2));
        assert_eq!(rot.to_string(), "rotate_cw");

        let tap4 = "tap_4".parse::<Gesture>().unwrap();
        assert_eq!(tap4, Gesture::new(GestureType::Tap, 4));
        assert_eq!(tap4.to_string(), "tap_4");
    }

    #[test]
    fn test_swipe_right_detection() {
        let mut recognizer = GestureRecognizer::new(GestureSensitivity::default());

        let f0 = TouchFrame::new(vec![
            TouchPoint { id: 1, x: 100.0, y: 100.0 },
            TouchPoint { id: 2, x: 120.0, y: 100.0 },
            TouchPoint { id: 3, x: 140.0, y: 100.0 },
        ]);
        assert_eq!(recognizer.process_frame(&f0), None);

        let f1 = TouchFrame::new(vec![
            TouchPoint { id: 1, x: 180.0, y: 102.0 },
            TouchPoint { id: 2, x: 200.0, y: 102.0 },
            TouchPoint { id: 3, x: 220.0, y: 102.0 },
        ]);
        let gesture = recognizer.process_frame(&f1);
        assert_eq!(gesture, Some(Gesture::new(GestureType::SwipeRight, 3)));
    }

    #[test]
    fn test_tap_detection() {
        let mut recognizer = GestureRecognizer::new(GestureSensitivity::default());

        let f0 = TouchFrame::new(vec![
            TouchPoint { id: 1, x: 100.0, y: 100.0 },
            TouchPoint { id: 2, x: 120.0, y: 100.0 },
            TouchPoint { id: 3, x: 140.0, y: 100.0 },
        ]);
        recognizer.process_frame(&f0);

        let f_lift = TouchFrame::new(vec![]);
        let gesture = recognizer.process_frame(&f_lift);
        assert_eq!(gesture, Some(Gesture::new(GestureType::Tap, 3)));
    }

    #[test]
    fn test_rotation_detection() {
        let mut recognizer = GestureRecognizer::new(GestureSensitivity::default());

        // Horizontal line of 2 fingers
        let f0 = TouchFrame::new(vec![
            TouchPoint { id: 1, x: 100.0, y: 100.0 },
            TouchPoint { id: 2, x: 140.0, y: 100.0 },
        ]);
        recognizer.process_frame(&f0);

        // Rotated clockwise (approx 45 degrees)
        let f1 = TouchFrame::new(vec![
            TouchPoint { id: 1, x: 100.0, y: 100.0 },
            TouchPoint { id: 2, x: 128.0, y: 128.0 },
        ]);
        let gesture = recognizer.process_frame(&f1);
        assert_eq!(gesture, Some(Gesture::new(GestureType::RotateCw, 2)));
    }
}
