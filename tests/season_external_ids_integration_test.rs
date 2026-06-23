//! Integration test for TMDB get_season_external_ids API.
//! Verifies that anthology series (e.g. Love, Death & Robots) return
//! correct season-level IMDB IDs.

use media_organizer::services::tmdb::TmdbClient;

const TV_SHOW_ID: u64 = 86831; // Love, Death & Robots

/// Test S01 and S04 IMDB IDs for anthology series "Love, Death & Robots"
///
/// Expected results:
/// - S01: tt9561862
/// - S04: tt21661768
#[tokio::test]
async fn test_season_external_ids_anthology_series() {
    let client = match TmdbClient::from_env() {
        Ok(c) => c,
        Err(_) => {
            eprintln!("SKIP: TMDB_API_KEY not set, skipping integration test");
            return;
        }
    };

    // Verify API key is valid
    let valid = client.verify_api_key().await.unwrap_or(false);
    assert!(valid, "TMDB API key is not valid");

    // Test S01
    let s1 = client.get_season_external_ids(TV_SHOW_ID, 1).await;
    println!("S01 result: {:?}", s1);
    match s1 {
        Ok(ref external) => {
            println!("S01 IMDB: {:?}", external.imdb_id);
            assert_eq!(external.imdb_id, Some("tt9561862".to_string()),
                "S01 should have IMDB ID tt9561862, got {:?}", external.imdb_id);
        }
        Err(e) => panic!("S01 API call failed: {}", e),
    }

    // Test S04
    let s4 = client.get_season_external_ids(TV_SHOW_ID, 4).await;
    println!("S04 result: {:?}", s4);
    match s4 {
        Ok(ref external) => {
            println!("S04 IMDB: {:?}", external.imdb_id);
            assert_eq!(external.imdb_id, Some("tt21661768".to_string()),
                "S04 should have IMDB ID tt21661768, got {:?}", external.imdb_id);
        }
        Err(e) => panic!("S04 API call failed: {}", e),
    }

    // Confirm they are DIFFERENT (anthology series property)
    let s1_imdb = s1.as_ref().unwrap().imdb_id.clone();
    let s4_imdb = s4.as_ref().unwrap().imdb_id.clone();
    assert_ne!(s1_imdb, s4_imdb,
        "Anthology series: S01 and S04 should have different IMDB IDs");
}

/// Test regular TV series where all seasons share the same IMDB ID
///
/// Expected: S01 and S02 should return the same IMDB ID (show-level)
#[tokio::test]
async fn test_season_external_ids_regular_series() {
    let client = match TmdbClient::from_env() {
        Ok(c) => c,
        Err(_) => {
            eprintln!("SKIP: TMDB_API_KEY not set, skipping integration test");
            return;
        }
    };

    // Breaking Bad: tmdb1396, show IMDB tt0903747
    let show_id: u64 = 1396;

    let s1 = client.get_season_external_ids(show_id, 1).await;
    let s2 = client.get_season_external_ids(show_id, 2).await;

    println!("Breaking Bad S01: {:?}", s1);
    println!("Breaking Bad S02: {:?}", s2);

    // For regular series, TMDB may or may not return season-specific IDs.
    // If TMDB returns None for seasons, the code falls back to show IMDB ID.
    // Both approaches are valid — we just verify the API itself works.
    match (s1, s2) {
        (Ok(s1_ext), Ok(s2_ext)) => {
            println!("S01 IMDB: {:?}, S02 IMDB: {:?}", s1_ext.imdb_id, s2_ext.imdb_id);
            // If TMDB returns IDs, they should match (same show)
            if let (Some(id1), Some(id2)) = (&s1_ext.imdb_id, &s2_ext.imdb_id) {
                assert_eq!(id1, id2, "Regular series: S01 and S02 should share IMDB ID");
            }
        }
        (Err(e1), _) => panic!("S01 API call failed: {}", e1),
        (_, Err(e2)) => panic!("S02 API call failed: {}", e2),
    }
}