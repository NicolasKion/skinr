//! SOF DNA, the string the Space Object Factory builds a ship from:
//! `hull:faction:race`, then the commands a material set adds (a ship's own
//! for its default livery, or a skin's).

/// A livery's DNA. `commands` is empty for a bare hull.
pub fn livery(hull: &str, faction: &str, race: &str, commands: &str) -> String {
    if commands.is_empty() {
        format!("{hull}:{faction}:{race}")
    } else {
        format!("{hull}:{faction}:{race}:{commands}")
    }
}

/// What a SKINR design looks like on its ship, as one string: the hull's
/// DNA and the design's layout. The worker turns the design into DNA itself
/// (`#lib/skinr.ts`), so this is what tells whether a drawing of it is
/// still current.
pub fn design_look(hull: &str, faction: &str, race: &str, layout: &str) -> String {
    format!("{}|{layout}", livery(hull, faction, race, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_design_look_changes_with_its_layout_and_hull() {
        let look = design_look("mf4_t1", "minmatarbase", "minmatar", r#"{"slots":[]}"#);
        assert_eq!(look, r#"mf4_t1:minmatarbase:minmatar|{"slots":[]}"#);
        assert_ne!(
            look,
            design_look("mf4_t1", "minmatarbase", "minmatar", "{}")
        );
    }

    #[test]
    fn appends_the_material_set_commands() {
        assert_eq!(
            livery("gb2_t2", "noh", "gallente", ""),
            "gb2_t2:noh:gallente"
        );
        assert_eq!(
            livery(
                "gb2_t2",
                "noh",
                "gallente",
                "pattern?headhunter_gallente;none;none"
            ),
            "gb2_t2:noh:gallente:pattern?headhunter_gallente;none;none"
        );
    }
}
