//! A ship's spec sheet: the dogma attributes that matter to a pilot, turned
//! from the SDE's raw values (milliseconds, resonances, multipliers) into the
//! figures the game's own Show Info window shows.

use std::collections::HashMap;

use serde::Serialize;

/// The dogma attributes the import keeps, by SDE name. Everything the sheet
/// reads must be listed here.
pub const SHEET_ATTRIBUTES: [&str; 46] = [
    // Fitting
    "hiSlots",
    "medSlots",
    "lowSlots",
    "upgradeSlotsLeft",
    "turretSlotsLeft",
    "launcherSlotsLeft",
    "powerOutput",
    "cpuOutput",
    "upgradeCapacity",
    "droneCapacity",
    "droneBandwidth",
    // Defense
    "shieldCapacity",
    "shieldRechargeRate",
    "shieldEmDamageResonance",
    "shieldThermalDamageResonance",
    "shieldKineticDamageResonance",
    "shieldExplosiveDamageResonance",
    "armorHP",
    "armorEmDamageResonance",
    "armorThermalDamageResonance",
    "armorKineticDamageResonance",
    "armorExplosiveDamageResonance",
    "hp",
    "emDamageResonance",
    "thermalDamageResonance",
    "kineticDamageResonance",
    "explosiveDamageResonance",
    // Capacitor
    "capacitorCapacity",
    "rechargeRate",
    // Targeting
    "maxTargetRange",
    "maxLockedTargets",
    "scanResolution",
    "signatureRadius",
    "scanRadarStrength",
    "scanLadarStrength",
    "scanMagnetometricStrength",
    "scanGravimetricStrength",
    // Navigation
    "maxVelocity",
    "agility",
    "baseWarpSpeed",
    "warpSpeedMultiplier",
    // Bays
    "fighterCapacity",
    "fighterTubes",
    "specialFuelBayCapacity",
    "specialOreHoldCapacity",
    "fleetHangarCapacity",
];

/// The time constant the game's align time uses: ln(2) times inertia
/// modifier times mass, over a million.
const ALIGN_MASS_SCALE: f64 = 1_000_000.0;

/// Dogma stores recharge times in milliseconds.
const MILLISECONDS: f64 = 1000.0;

/// Dogma stores ranges in metres; the sheet shows targeting range in km.
const METRES_PER_KM: f64 = 1000.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sheet {
    pub slots: Slots,
    pub fitting: Fitting,
    pub defense: Vec<Layer>,
    pub capacitor: Capacitor,
    pub targeting: Targeting,
    pub navigation: Navigation,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Slots {
    pub high: u32,
    pub mid: u32,
    pub low: u32,
    pub rig: u32,
    pub turret: u32,
    pub launcher: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fitting {
    /// MW.
    pub powergrid: Option<f64>,
    /// teraflops.
    pub cpu: Option<f64>,
    pub calibration: Option<f64>,
    /// m³.
    pub drone_bay: Option<f64>,
    /// Mbit/s.
    pub drone_bandwidth: Option<f64>,
    /// m³.
    pub fighter_bay: Option<f64>,
    pub fighter_tubes: Option<u32>,
}

/// One defensive layer and its resistances in percent (0 to 100).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Layer {
    pub name: &'static str,
    pub hp: f64,
    pub em: f64,
    pub thermal: f64,
    pub kinetic: f64,
    pub explosive: f64,
    /// Seconds, for the shield only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recharge: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Capacitor {
    /// GJ.
    pub capacity: Option<f64>,
    /// Seconds.
    pub recharge: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Targeting {
    /// km.
    pub range: Option<f64>,
    pub max_targets: Option<u32>,
    /// mm.
    pub scan_resolution: Option<f64>,
    /// m.
    pub signature_radius: Option<f64>,
    /// The ship's sensor type and its strength in points.
    pub sensor: Option<Sensor>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sensor {
    pub kind: &'static str,
    pub strength: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Navigation {
    /// m/s.
    pub max_velocity: Option<f64>,
    /// AU/s.
    pub warp_speed: Option<f64>,
    /// Seconds to align for warp.
    pub align_time: Option<f64>,
    pub inertia: Option<f64>,
}

/// Builds the sheet from a ship's attribute values (by name) and its mass.
pub fn build(values: &HashMap<String, f64>, mass: Option<f64>) -> Sheet {
    let get = |name: &str| values.get(name).copied();
    let count = |name: &str| get(name).map(|value| value.max(0.0).round() as u32);
    let positive = |value: Option<f64>| value.filter(|value| *value > 0.0);
    // A resonance of 0.6 lets 60% through: a 40% resistance.
    let resist = |name: &str| (1.0 - get(name).unwrap_or(1.0)) * 100.0;

    // Resonances are listed EM, thermal, kinetic, explosive.
    let layer =
        |name: &'static str, hp: &str, resonances: [&str; 4], recharge: Option<f64>| Layer {
            name,
            hp: get(hp).unwrap_or(0.0),
            em: resist(resonances[0]),
            thermal: resist(resonances[1]),
            kinetic: resist(resonances[2]),
            explosive: resist(resonances[3]),
            recharge,
        };

    let sensor = [
        ("Radar", "scanRadarStrength"),
        ("Ladar", "scanLadarStrength"),
        ("Magnetometric", "scanMagnetometricStrength"),
        ("Gravimetric", "scanGravimetricStrength"),
    ]
    .into_iter()
    .filter_map(|(kind, name)| Some((kind, positive(get(name))?)))
    .max_by(|a, b| a.1.total_cmp(&b.1))
    .map(|(kind, strength)| Sensor { kind, strength });

    let agility = positive(get("agility"));

    Sheet {
        slots: Slots {
            high: count("hiSlots").unwrap_or(0),
            mid: count("medSlots").unwrap_or(0),
            low: count("lowSlots").unwrap_or(0),
            rig: count("upgradeSlotsLeft").unwrap_or(0),
            turret: count("turretSlotsLeft").unwrap_or(0),
            launcher: count("launcherSlotsLeft").unwrap_or(0),
        },
        fitting: Fitting {
            powergrid: positive(get("powerOutput")),
            cpu: positive(get("cpuOutput")),
            calibration: positive(get("upgradeCapacity")),
            drone_bay: positive(get("droneCapacity")),
            drone_bandwidth: positive(get("droneBandwidth")),
            fighter_bay: positive(get("fighterCapacity")),
            fighter_tubes: count("fighterTubes").filter(|tubes| *tubes > 0),
        },
        defense: vec![
            layer(
                "Shield",
                "shieldCapacity",
                [
                    "shieldEmDamageResonance",
                    "shieldThermalDamageResonance",
                    "shieldKineticDamageResonance",
                    "shieldExplosiveDamageResonance",
                ],
                positive(get("shieldRechargeRate")).map(|ms| ms / MILLISECONDS),
            ),
            layer(
                "Armor",
                "armorHP",
                [
                    "armorEmDamageResonance",
                    "armorThermalDamageResonance",
                    "armorKineticDamageResonance",
                    "armorExplosiveDamageResonance",
                ],
                None,
            ),
            layer(
                "Structure",
                "hp",
                [
                    "emDamageResonance",
                    "thermalDamageResonance",
                    "kineticDamageResonance",
                    "explosiveDamageResonance",
                ],
                None,
            ),
        ],
        capacitor: Capacitor {
            capacity: positive(get("capacitorCapacity")),
            recharge: positive(get("rechargeRate")).map(|ms| ms / MILLISECONDS),
        },
        targeting: Targeting {
            range: positive(get("maxTargetRange")).map(|m| m / METRES_PER_KM),
            max_targets: count("maxLockedTargets"),
            scan_resolution: positive(get("scanResolution")),
            signature_radius: positive(get("signatureRadius")),
            sensor,
        },
        navigation: Navigation {
            max_velocity: positive(get("maxVelocity")),
            warp_speed: positive(get("baseWarpSpeed"))
                .map(|base| base * get("warpSpeedMultiplier").unwrap_or(1.0)),
            align_time: agility
                .zip(positive(mass))
                .map(|(agility, mass)| std::f64::consts::LN_2 * agility * mass / ALIGN_MASS_SCALE),
            inertia: agility,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Merlin's SDE values.
    fn merlin() -> HashMap<String, f64> {
        [
            ("hiSlots", 3.0),
            ("medSlots", 4.0),
            ("lowSlots", 3.0),
            ("upgradeSlotsLeft", 3.0),
            ("turretSlotsLeft", 3.0),
            ("launcherSlotsLeft", 0.0),
            ("powerOutput", 40.0),
            ("cpuOutput", 180.0),
            ("upgradeCapacity", 400.0),
            ("droneCapacity", 0.0),
            ("shieldCapacity", 500.0),
            ("shieldRechargeRate", 625000.0),
            ("shieldEmDamageResonance", 1.0),
            ("shieldThermalDamageResonance", 0.8),
            ("shieldKineticDamageResonance", 0.6),
            ("shieldExplosiveDamageResonance", 0.5),
            ("armorHP", 350.0),
            ("armorEmDamageResonance", 0.5),
            ("armorThermalDamageResonance", 0.55),
            ("armorKineticDamageResonance", 0.75),
            ("armorExplosiveDamageResonance", 0.9),
            ("hp", 400.0),
            ("emDamageResonance", 0.67),
            ("thermalDamageResonance", 0.67),
            ("kineticDamageResonance", 0.67),
            ("explosiveDamageResonance", 0.67),
            ("capacitorCapacity", 350.0),
            ("rechargeRate", 175000.0),
            ("maxTargetRange", 50000.0),
            ("maxLockedTargets", 5.0),
            ("scanResolution", 580.0),
            ("signatureRadius", 39.0),
            ("scanRadarStrength", 0.0),
            ("scanGravimetricStrength", 11.0),
            ("maxVelocity", 320.0),
            ("agility", 3.6),
            ("baseWarpSpeed", 1.0),
            ("warpSpeedMultiplier", 5.0),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn slots_and_fitting() {
        let sheet = build(&merlin(), Some(997000.0));
        assert_eq!(
            sheet.slots,
            Slots {
                high: 3,
                mid: 4,
                low: 3,
                rig: 3,
                turret: 3,
                launcher: 0
            }
        );
        assert_eq!(sheet.fitting.powergrid, Some(40.0));
        assert_eq!(sheet.fitting.cpu, Some(180.0));
        assert_eq!(sheet.fitting.calibration, Some(400.0));
        // A zero capacity is no drone bay at all.
        assert_eq!(sheet.fitting.drone_bay, None);
        assert_eq!(sheet.fitting.fighter_tubes, None);
    }

    #[test]
    fn resonances_become_resistances() {
        let sheet = build(&merlin(), None);
        let shield = &sheet.defense[0];
        assert_eq!(shield.name, "Shield");
        assert!(close(shield.em, 0.0));
        assert!(close(shield.thermal, 20.0));
        assert!(close(shield.kinetic, 40.0));
        assert!(close(shield.explosive, 50.0));
        assert_eq!(shield.recharge, Some(625.0));
        let structure = &sheet.defense[2];
        assert_eq!(structure.name, "Structure");
        assert!(close(structure.em, 33.0));
        assert_eq!(structure.recharge, None);
    }

    #[test]
    fn targeting_and_navigation() {
        let sheet = build(&merlin(), Some(997000.0));
        assert_eq!(sheet.targeting.range, Some(50.0));
        assert_eq!(
            sheet.targeting.sensor,
            Some(Sensor {
                kind: "Gravimetric",
                strength: 11.0
            })
        );
        assert_eq!(sheet.capacitor.recharge, Some(175.0));
        assert_eq!(sheet.navigation.warp_speed, Some(5.0));
        // ln 2 x 3.6 x 997000 / 1e6
        assert!(close(
            sheet.navigation.align_time.unwrap(),
            std::f64::consts::LN_2 * 3.6 * 0.997
        ));
        // No mass, no align time.
        assert_eq!(build(&merlin(), None).navigation.align_time, None);
    }

    #[test]
    fn every_name_the_sheet_reads_is_imported() {
        let source = include_str!("sheet.rs");
        let body = source.split("pub fn build").nth(1).unwrap();
        let body = body.split("#[cfg(test)]").next().unwrap();
        for quoted in body.split('"').skip(1).step_by(2) {
            if quoted.chars().all(|c| c.is_ascii_alphanumeric())
                && quoted.starts_with(char::is_lowercase)
            {
                assert!(
                    SHEET_ATTRIBUTES.contains(&quoted),
                    "{quoted} is read but not imported"
                );
            }
        }
    }
}
