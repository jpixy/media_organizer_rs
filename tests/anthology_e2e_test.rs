//! E2E test for anthology series season IMDB ID re-organization.
//! Uses real data from disk and TMDB API.
//! Note: This is an integration test that requires specific test data.

use media_organizer::core::planner::Planner;
use media_organizer::models::media::MediaType;

const SOURCE: &str = "/run/media/johnny/JMedia_S02/johnny/Media/TV_00_TMP/EN_English/[A][爱，死亡和机器人][Love, Death & Robots](2025)-tmdb450504/[S04][Season 04]-[A][爱，死亡和机器人][Love, Death & Robots]-tmdb450504";
const TARGET: &str = "/tmp/test_output";

#[tokio::test]
async fn test_anthology_season_imdb_id_e2e() {
    let planner = match Planner::new() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("SKIP: Could not create planner (TMDB_API_KEY may be missing)");
            return;
        }
    };

    let source = std::path::Path::new(SOURCE);
    let target = std::path::Path::new(TARGET);

    // Skip if source path doesn't exist (environment-specific)
    if !source.exists() {
        eprintln!("SKIP: Source path does not exist: {}", SOURCE);
        return;
    }

    println!("Source: {:?}", SOURCE);
    println!("Target: {:?}", TARGET);

    let plan = match planner.generate(source, target, MediaType::TvSeries).await {
        Ok(p) => p,
        Err(e) => {
            panic!("Plan generation failed: {}", e);
        }
    };

    println!("Plan items: {} ({} pending, {} skip, {} error)",
        plan.items.len(),
        plan.items.iter().filter(|i| i.status == media_organizer::models::plan::PlanItemStatus::Pending).count(),
        plan.items.iter().filter(|i| i.status == media_organizer::models::plan::PlanItemStatus::Skip).count(),
        plan.items.iter().filter(|i| i.status == media_organizer::models::plan::PlanItemStatus::Error).count(),
    );
    println!("Unknown items: {}", plan.unknown.len());
    
    // Print all items (including skipped/errored)
    for item in &plan.items {
        println!("  [{:?}] folder: {} | season_imdb: {:?}", item.status, item.target.folder,
            item.season_metadata.as_ref().and_then(|s| s.imdb_id.as_deref()));
        if let Some(ref season) = item.season_metadata {
            if season.season_number == 4 {
                assert_eq!(season.imdb_id, Some("tt21661768".to_string()),
                    "S04 should have IMDB ID tt21661768, got {:?}", season.imdb_id);
                assert!(item.target.folder.contains("tt21661768"),
                    "Output folder should contain tt21661768, got: {}", item.target.folder);
                println!("    ✓ S04 has correct IMDB ID: tt21661768");
            }
        }
    }
    
    // Print unknown items
    for unknown in &plan.unknown {
        println!("  [UNKNOWN] {} - reason: {}", unknown.source.filename, unknown.reason);
    }
    
    // Verify we got at least some items
    assert!(!plan.items.is_empty() || !plan.unknown.is_empty(),
        "Expected at least some plan items or unknown items, got {} items and {} unknown",
        plan.items.len(), plan.unknown.len());
}