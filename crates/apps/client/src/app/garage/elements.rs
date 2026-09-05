//! The garage overlay's element names (interface program F5): each panel one element on the
//! draw list, its legacy vertices carried verbatim until the G wave restyles it.

/// Append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GarageElement {
    TopBar,
    Nameplate,
    Crew,
    Stats,
    Loadout,
    Carousel,
    InspectorLegend,
    Options,
    TechTree,
    /// The wash over whatever the cursor is on.
    Hover,
}

impl GarageElement {
    /// Walked by the tests and by the G wave's screen census; the identity rule wants it whole.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: [GarageElement; 10] = [
        GarageElement::TopBar,
        GarageElement::Nameplate,
        GarageElement::Crew,
        GarageElement::Stats,
        GarageElement::Loadout,
        GarageElement::Carousel,
        GarageElement::InspectorLegend,
        GarageElement::Options,
        GarageElement::TechTree,
        GarageElement::Hover,
    ];
}

#[cfg(test)]
mod tests {
    use super::GarageElement;

    #[test]
    fn every_element_is_named_once() {
        let names: std::collections::HashSet<String> =
            GarageElement::ALL.iter().map(|e| format!("{e:?}")).collect();
        assert_eq!(names.len(), GarageElement::ALL.len());
    }
}
