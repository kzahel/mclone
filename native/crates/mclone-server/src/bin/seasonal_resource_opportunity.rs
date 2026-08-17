use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use mclone_season::{
    MCLONE_CALENDAR_DAYS_PER_YEAR, SEASONAL_RESOURCE_FACTOR_SCALE,
    SEASONAL_RESOURCE_RESPONSE_REVISION,
};
use mclone_server::{
    DimensionDefinition, LocalRealmSession, MemoryWorldStore, SeasonalWildlifeResourceSample,
    WILDLIFE_RESOURCE_RULE_REVISION, WildlifeForageCellPos, WildlifeResourceKind,
    WorldGenerationProfile,
};
use serde::Serialize;

type AnyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Clone, Debug)]
struct Args {
    seed: i64,
    output: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u32,
    seed: i64,
    calendar_days_per_year: u16,
    response_revision: u32,
    resource_rule_revision: u32,
    factor_scale: u16,
    illustrative_standing_stock: u32,
    sites: Vec<SiteReport>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteReport {
    label: String,
    position: WildlifeForageCellPos,
    dates: Vec<DateReport>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DateReport {
    day_of_year: u16,
    samples: Vec<OpportunityReport>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpportunityReport {
    #[serde(flatten)]
    sample: SeasonalWildlifeResourceSample,
    effective_accessible_per_10_000_standing: u32,
}

fn main() -> AnyResult<()> {
    let args = parse_args()?;
    let definition =
        DimensionDefinition::overworld(args.seed, WorldGenerationProfile::McloneOverworldV1);
    let mut server = LocalRealmSession::local_integrated_with_world_store_and_dimension_definition(
        definition,
        Box::new(MemoryWorldStore::new()),
    );

    let mut sites = vec![
        (
            "tropical-origin".to_owned(),
            WildlifeForageCellPos { x: 0, z: 0 },
        ),
        (
            "northern-temperate".to_owned(),
            WildlifeForageCellPos { x: 0, z: 128 },
        ),
        (
            "southern-temperate".to_owned(),
            WildlifeForageCellPos { x: 0, z: -129 },
        ),
    ];
    sites.extend(select_climate_bookends(&server)?);

    let mut site_reports = Vec::new();
    for (label, position) in sites {
        let mut dates = Vec::new();
        for day_of_year in [1_u16, 15, 29, 43] {
            server.set_calendar_date(0, day_of_year, Some(0))?;
            let mut samples = Vec::new();
            for resource in WildlifeResourceKind::ALL {
                let sample = server
                    .seasonal_wildlife_resource_sample(position, resource)
                    .ok_or("Mclone calendar unexpectedly produced no seasonal resource sample")?;
                samples.push(OpportunityReport {
                    effective_accessible_per_10_000_standing: u32::from(
                        sample.accessibility_basis_points,
                    ),
                    sample,
                });
            }
            dates.push(DateReport {
                day_of_year,
                samples,
            });
        }
        site_reports.push(SiteReport {
            label,
            position,
            dates,
        });
    }

    let report = Report {
        schema_version: 1,
        seed: args.seed,
        calendar_days_per_year: MCLONE_CALENDAR_DAYS_PER_YEAR,
        response_revision: SEASONAL_RESOURCE_RESPONSE_REVISION,
        resource_rule_revision: WILDLIFE_RESOURCE_RULE_REVISION,
        factor_scale: SEASONAL_RESOURCE_FACTOR_SCALE,
        illustrative_standing_stock: u32::from(SEASONAL_RESOURCE_FACTOR_SCALE),
        sites: site_reports,
    };
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serde_json::to_vec_pretty(&report)?)?;
    println!("wrote {}", args.output.display());
    Ok(())
}

fn select_climate_bookends(
    server: &LocalRealmSession,
) -> AnyResult<Vec<(String, WildlifeForageCellPos)>> {
    let mut wettest: Option<SeasonalWildlifeResourceSample> = None;
    let mut driest: Option<SeasonalWildlifeResourceSample> = None;
    let mut highest: Option<SeasonalWildlifeResourceSample> = None;
    let mut coldest: Option<SeasonalWildlifeResourceSample> = None;
    for x in (-256..=256).step_by(16) {
        for z in (-256..=256).step_by(16) {
            let sample = server
                .seasonal_wildlife_resource_sample(
                    WildlifeForageCellPos { x, z },
                    WildlifeResourceKind::LowHerbaceous,
                )
                .ok_or("climate scan produced no seasonal resource sample")?;
            if wettest
                .is_none_or(|current| sample.moisture_basis_points > current.moisture_basis_points)
            {
                wettest = Some(sample);
            }
            if driest
                .is_none_or(|current| sample.moisture_basis_points < current.moisture_basis_points)
            {
                driest = Some(sample);
            }
            if highest.is_none_or(|current| sample.surface_y > current.surface_y) {
                highest = Some(sample);
            }
            if coldest.is_none_or(|current| {
                sample.mean_temperature_basis_points < current.mean_temperature_basis_points
            }) {
                coldest = Some(sample);
            }
        }
    }
    Ok([
        ("wet-climate", wettest),
        ("dry-climate", driest),
        ("high-elevation", highest),
        ("cold-climate", coldest),
    ]
    .into_iter()
    .map(|(label, sample)| {
        (
            label.to_owned(),
            sample
                .expect("non-empty deterministic climate scan")
                .position,
        )
    })
    .collect())
}

fn parse_args() -> AnyResult<Args> {
    let mut seed = 12_345_i64;
    let mut output = PathBuf::from("/tmp/mclone-seasonal-resource-opportunity.json");
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => seed = args.next().ok_or("--seed requires a value")?.parse()?,
            "--output" => output = PathBuf::from(args.next().ok_or("--output requires a path")?),
            "--help" | "-h" => {
                println!(
                    "seasonal_resource_opportunity [--seed 12345] [--output /tmp/report.json]"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    Ok(Args { seed, output })
}
