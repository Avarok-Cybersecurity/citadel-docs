//! model → ProseMirror JSON → model is the identity, on generated documents and the kitchen
//! sink; and everything the encoder emits conforms to the generated schema.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::test_support::strategies;
use cdoc_pm::schema::{check, schema};
use cdoc_pm::{from_pm, from_pm_json, to_pm, to_pm_json, PmNode};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn round_trip_is_identity(doc in strategies::document()) {
        let pm = to_pm(&doc);
        let back = from_pm(&pm).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(back, doc);
    }

    #[test]
    fn round_trip_through_json_text_is_identity(doc in strategies::document()) {
        let text = to_pm_json(&doc);
        let back = from_pm_json(&text).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(back, doc);
    }

    #[test]
    fn encoded_documents_conform_to_the_schema(doc in strategies::document()) {
        check(&to_pm(&doc), &schema()).map_err(|e| TestCaseError::fail(e.to_string()))?;
    }

    #[test]
    fn encoding_is_stable(doc in strategies::document()) {
        // The same document always yields byte-identical JSON (no map-order or float noise),
        // which the collaboration layer relies on to detect "no change".
        prop_assert_eq!(to_pm_json(&doc), to_pm_json(&doc.clone()));
    }
}

#[test]
fn kitchen_sink_round_trips_and_conforms() {
    let doc = kitchen_sink();
    let pm = to_pm(&doc);
    check(&pm, &schema()).unwrap();
    assert_eq!(from_pm(&pm).unwrap(), doc);
    let text = to_pm_json(&doc);
    let reparsed: PmNode = serde_json::from_str(&text).unwrap();
    assert_eq!(reparsed, pm);
}

#[test]
fn kitchen_sink_json_has_the_documented_shape() {
    let pm = serde_json::to_value(to_pm(&kitchen_sink())).unwrap();
    assert_eq!(pm["type"], "doc");
    assert_eq!(pm["attrs"]["meta"]["title"], "Kitchen sink");
    assert!(pm["attrs"]["styles"]["styles"].as_array().unwrap().len() >= 10);
    let section = &pm["content"][0];
    assert_eq!(section["type"], "section");
    assert_eq!(section["attrs"]["page"]["size"]["orientation"], "landscape");
    assert_eq!(section["attrs"]["titlePage"], true);
    assert_eq!(section["content"][0]["type"], "header");
    assert_eq!(section["content"][0]["attrs"]["kind"], "default");
    assert_eq!(section["content"][3]["type"], "footer");
    let title = &section["content"][4];
    assert_eq!(title["type"], "paragraph");
    assert_eq!(title["attrs"]["style"], "Title");
    let rich = &section["content"][6]["content"];
    let link = &rich[1];
    assert_eq!(link["type"], "text");
    let marks: Vec<&str> = link["marks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        marks,
        [
            "characterStyle",
            "textStyle",
            "bold",
            "italic",
            "underline",
            "strike",
            "verticalAlign",
            "highlight",
            "smallCaps",
            "allCaps",
            "kern",
            "ligatures",
            "lang",
            "link",
            "comment",
            "comment",
            "insertion"
        ]
    );
    assert_eq!(
        link["marks"][5]["attrs"]["on"], false,
        "strike is explicitly off"
    );
    assert_eq!(link["marks"][1]["attrs"]["fontSize"], 28);
    let table = &section["content"][8];
    assert_eq!(table["type"], "table");
    assert_eq!(table["content"][0]["content"][0]["type"], "tableHeader");
    assert_eq!(table["content"][0]["content"][0]["attrs"]["colspan"], 2);
    assert_eq!(
        table["content"][1]["content"][1]["content"][0]["type"],
        "table"
    );
    let last = pm["content"].as_array().unwrap().last().unwrap();
    assert_eq!(last["type"], "footnotes");
    assert_eq!(last["content"][0]["attrs"]["id"], 1);
}
