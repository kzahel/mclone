use std::{error::Error, fmt};

use mclone_render::uniform::MAX_PRESENTATION_VIEW_COUNT;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelExtent {
    pub width: u32,
    pub height: u32,
}

impl PixelExtent {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SafeAreaInsets {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl SafeAreaInsets {
    pub const NONE: Self = Self {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn extent(self) -> PixelExtent {
        PixelExtent::new(self.width, self.height)
    }

    fn right(self) -> Option<u32> {
        self.x.checked_add(self.width)
    }

    fn bottom(self) -> Option<u32> {
        self.y.checked_add(self.height)
    }

    fn overlaps(self, other: Self) -> bool {
        self.x < other.right().expect("validated rectangle")
            && other.x < self.right().expect("validated rectangle")
            && self.y < other.bottom().expect("validated rectangle")
            && other.y < self.bottom().expect("validated rectangle")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlatSurfaceLayoutError {
    EmptySurface,
    InvalidSafeArea,
    EmptyPanes,
    TooManyPanes { count: usize, maximum: usize },
    EmptyPane { index: usize },
    PaneOverflow { index: usize },
    PaneOutsideSafeArea { index: usize },
    OverlappingPanes { first: usize, second: usize },
}

impl fmt::Display for FlatSurfaceLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySurface => write!(formatter, "flat presentation surface must be non-empty"),
            Self::InvalidSafeArea => write!(formatter, "safe-area insets consume the surface"),
            Self::EmptyPanes => write!(formatter, "flat presentation requires at least one pane"),
            Self::TooManyPanes { count, maximum } => {
                write!(
                    formatter,
                    "flat presentation has {count} panes; maximum is {maximum}"
                )
            }
            Self::EmptyPane { index } => write!(formatter, "pane {index} is empty"),
            Self::PaneOverflow { index } => write!(formatter, "pane {index} overflows pixel space"),
            Self::PaneOutsideSafeArea { index } => {
                write!(formatter, "pane {index} lies outside the safe area")
            }
            Self::OverlappingPanes { first, second } => {
                write!(formatter, "panes {first} and {second} overlap")
            }
        }
    }
}

impl Error for FlatSurfaceLayoutError {}

/// Validated target-neutral pane rectangles for one flat presentation surface.
///
/// Pane order maps directly to neutral presentation-view order. Rectangles may
/// leave unused space but must be non-empty, non-overlapping, and contained by
/// the declared safe area.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatSurfaceLayout {
    surface: PixelExtent,
    safe_area: SafeAreaInsets,
    content: PixelRect,
    panes: Vec<PixelRect>,
}

impl FlatSurfaceLayout {
    pub fn new(
        surface: PixelExtent,
        safe_area: SafeAreaInsets,
        panes: impl IntoIterator<Item = PixelRect>,
    ) -> Result<Self, FlatSurfaceLayoutError> {
        if surface.width == 0 || surface.height == 0 {
            return Err(FlatSurfaceLayoutError::EmptySurface);
        }
        let Some(horizontal_insets) = safe_area.left.checked_add(safe_area.right) else {
            return Err(FlatSurfaceLayoutError::InvalidSafeArea);
        };
        let Some(vertical_insets) = safe_area.top.checked_add(safe_area.bottom) else {
            return Err(FlatSurfaceLayoutError::InvalidSafeArea);
        };
        let Some(content_width) = surface.width.checked_sub(horizontal_insets) else {
            return Err(FlatSurfaceLayoutError::InvalidSafeArea);
        };
        let Some(content_height) = surface.height.checked_sub(vertical_insets) else {
            return Err(FlatSurfaceLayoutError::InvalidSafeArea);
        };
        if content_width == 0 || content_height == 0 {
            return Err(FlatSurfaceLayoutError::InvalidSafeArea);
        }
        let content = PixelRect::new(safe_area.left, safe_area.top, content_width, content_height);
        let panes = panes.into_iter().collect::<Vec<_>>();
        if panes.is_empty() {
            return Err(FlatSurfaceLayoutError::EmptyPanes);
        }
        if panes.len() > MAX_PRESENTATION_VIEW_COUNT as usize {
            return Err(FlatSurfaceLayoutError::TooManyPanes {
                count: panes.len(),
                maximum: MAX_PRESENTATION_VIEW_COUNT as usize,
            });
        }
        let content_right = content.right().expect("validated content rectangle");
        let content_bottom = content.bottom().expect("validated content rectangle");
        for (index, pane) in panes.iter().copied().enumerate() {
            if pane.width == 0 || pane.height == 0 {
                return Err(FlatSurfaceLayoutError::EmptyPane { index });
            }
            let Some(right) = pane.right() else {
                return Err(FlatSurfaceLayoutError::PaneOverflow { index });
            };
            let Some(bottom) = pane.bottom() else {
                return Err(FlatSurfaceLayoutError::PaneOverflow { index });
            };
            if pane.x < content.x
                || pane.y < content.y
                || right > content_right
                || bottom > content_bottom
            {
                return Err(FlatSurfaceLayoutError::PaneOutsideSafeArea { index });
            }
        }
        for first in 0..panes.len() {
            for second in first + 1..panes.len() {
                if panes[first].overlaps(panes[second]) {
                    return Err(FlatSurfaceLayoutError::OverlappingPanes { first, second });
                }
            }
        }
        Ok(Self {
            surface,
            safe_area,
            content,
            panes,
        })
    }

    /// Two panes arranged left-to-right across the horizontal surface axis.
    pub fn two_horizontal(
        surface: PixelExtent,
        safe_area: SafeAreaInsets,
    ) -> Result<Self, FlatSurfaceLayoutError> {
        let content = content_rect(surface, safe_area)?;
        let first_width = content.width.div_ceil(2);
        let second_width = content.width - first_width;
        Self::new(
            surface,
            safe_area,
            [
                PixelRect::new(content.x, content.y, first_width, content.height),
                PixelRect::new(
                    content.x + first_width,
                    content.y,
                    second_width,
                    content.height,
                ),
            ],
        )
    }

    /// Two panes arranged top-to-bottom across the vertical surface axis.
    pub fn two_vertical(
        surface: PixelExtent,
        safe_area: SafeAreaInsets,
    ) -> Result<Self, FlatSurfaceLayoutError> {
        let content = content_rect(surface, safe_area)?;
        let first_height = content.height.div_ceil(2);
        let second_height = content.height - first_height;
        Self::new(
            surface,
            safe_area,
            [
                PixelRect::new(content.x, content.y, content.width, first_height),
                PixelRect::new(
                    content.x,
                    content.y + first_height,
                    content.width,
                    second_height,
                ),
            ],
        )
    }

    pub const fn surface(&self) -> PixelExtent {
        self.surface
    }

    pub const fn safe_area(&self) -> SafeAreaInsets {
        self.safe_area
    }

    pub const fn content_rect(&self) -> PixelRect {
        self.content
    }

    pub fn panes(&self) -> &[PixelRect] {
        &self.panes
    }
}

fn content_rect(
    surface: PixelExtent,
    safe_area: SafeAreaInsets,
) -> Result<PixelRect, FlatSurfaceLayoutError> {
    FlatSurfaceLayout::new(
        surface,
        safe_area,
        [PixelRect::new(
            safe_area.left,
            safe_area.top,
            surface
                .width
                .saturating_sub(safe_area.left)
                .saturating_sub(safe_area.right),
            surface
                .height
                .saturating_sub(safe_area.top)
                .saturating_sub(safe_area.bottom),
        )],
    )
    .map(|layout| layout.content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_and_vertical_two_pane_layouts_cover_odd_safe_area() {
        let surface = PixelExtent::new(101, 81);
        let safe = SafeAreaInsets {
            left: 3,
            top: 5,
            right: 4,
            bottom: 6,
        };
        let horizontal = FlatSurfaceLayout::two_horizontal(surface, safe).unwrap();
        assert_eq!(
            horizontal.panes(),
            [PixelRect::new(3, 5, 47, 70), PixelRect::new(50, 5, 47, 70)]
        );
        let vertical = FlatSurfaceLayout::two_vertical(surface, safe).unwrap();
        assert_eq!(
            vertical.panes(),
            [PixelRect::new(3, 5, 94, 35), PixelRect::new(3, 40, 94, 35)]
        );
    }

    #[test]
    fn arbitrary_one_to_four_panes_validate_without_pair_assumption() {
        let surface = PixelExtent::new(400, 300);
        for count in 1..=MAX_PRESENTATION_VIEW_COUNT as usize {
            let panes = (0..count)
                .map(|index| PixelRect::new(index as u32 * 100, 0, 100, 100))
                .collect::<Vec<_>>();
            assert_eq!(
                FlatSurfaceLayout::new(surface, SafeAreaInsets::NONE, panes)
                    .unwrap()
                    .panes()
                    .len(),
                count
            );
        }
        assert!(matches!(
            FlatSurfaceLayout::new(
                PixelExtent::new(500, 100),
                SafeAreaInsets::NONE,
                (0..5).map(|index| PixelRect::new(index * 100, 0, 100, 100))
            ),
            Err(FlatSurfaceLayoutError::TooManyPanes { .. })
        ));
    }

    #[test]
    fn layout_rejects_empty_outside_and_overlapping_panes() {
        let surface = PixelExtent::new(100, 100);
        let safe = SafeAreaInsets {
            left: 10,
            top: 10,
            right: 10,
            bottom: 10,
        };
        assert_eq!(
            FlatSurfaceLayout::new(surface, safe, [PixelRect::new(10, 10, 0, 20)]),
            Err(FlatSurfaceLayoutError::EmptyPane { index: 0 })
        );
        assert_eq!(
            FlatSurfaceLayout::new(surface, safe, [PixelRect::new(5, 10, 20, 20)]),
            Err(FlatSurfaceLayoutError::PaneOutsideSafeArea { index: 0 })
        );
        assert_eq!(
            FlatSurfaceLayout::new(
                surface,
                safe,
                [
                    PixelRect::new(10, 10, 40, 40),
                    PixelRect::new(49, 20, 20, 20)
                ]
            ),
            Err(FlatSurfaceLayoutError::OverlappingPanes {
                first: 0,
                second: 1
            })
        );
    }
}
