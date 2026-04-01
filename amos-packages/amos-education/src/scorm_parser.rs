//! SCORM manifest parser.
//!
//! Parses `imsmanifest.xml` from SCORM 1.2 and SCORM 2004 packages
//! to extract course metadata, SCO structure, and launch information.

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::io::Read;

/// Parsed SCORM manifest data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScormManifest {
    /// SCORM version detected ("1.2" or "2004")
    pub scorm_version: String,
    /// Course title from the manifest
    pub title: String,
    /// Course description (if present)
    pub description: Option<String>,
    /// Default organization identifier
    pub default_org: Option<String>,
    /// Shareable Content Objects in this package
    pub scos: Vec<ScormSco>,
    /// The primary launch URL (first SCO's href)
    pub launch_url: String,
    /// Raw metadata from the manifest
    pub metadata: ScormMetadata,
}

/// A Shareable Content Object (SCO) — one launchable unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScormSco {
    /// SCO identifier from the manifest
    pub identifier: String,
    /// SCO title
    pub title: String,
    /// Launch URL relative to package root
    pub href: String,
    /// SCORM type: "sco" or "asset"
    pub sco_type: String,
    /// Mastery score (if defined via adlcp:masteryscore or sequencing)
    pub mastery_score: Option<f64>,
    /// Prerequisites (SCORM 1.2 style)
    pub prerequisites: Option<String>,
    /// Max time allowed
    pub max_time_allowed: Option<String>,
    /// Time limit action
    pub time_limit_action: Option<String>,
}

/// Manifest-level metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScormMetadata {
    pub schema: Option<String>,
    pub schema_version: Option<String>,
    pub identifier: Option<String>,
}

/// Parse a SCORM manifest from XML bytes.
pub fn parse_manifest(xml_bytes: &[u8]) -> Result<ScormManifest, String> {
    let mut reader = Reader::from_reader(xml_bytes);
    reader.config_mut().trim_text(true);

    let mut manifest = ScormManifest {
        scorm_version: "1.2".to_string(),
        title: String::new(),
        description: None,
        default_org: None,
        scos: Vec::new(),
        launch_url: String::new(),
        metadata: ScormMetadata::default(),
    };

    // Resource map: identifier -> href (populated from <resources> section)
    let mut resources: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // Pending items — resolved after full parse since <resources> comes after <organizations>
    struct PendingItem {
        identifier: String,
        title: String,
        identifierref: String,
        mastery_score: Option<f64>,
        prerequisites: Option<String>,
    }
    let mut pending_items: Vec<PendingItem> = Vec::new();

    // Parsing state
    let mut buf = Vec::new();
    let mut in_metadata = false;
    let mut in_organizations = false;
    let mut in_item = false;
    let mut in_resources = false;
    let mut current_text = String::new();
    let mut current_item_id = String::new();
    let mut current_item_title = String::new();
    let mut current_item_ref = String::new();
    let mut current_mastery: Option<f64> = None;
    let mut current_prerequisites: Option<String> = None;
    let mut depth = 0u32;
    let mut title_depth = 0u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();

                match name.as_str() {
                    "manifest" => {
                        // Extract manifest identifier
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"identifier" {
                                manifest.metadata.identifier =
                                    Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                    }
                    "metadata" if depth == 2 => {
                        in_metadata = true;
                    }
                    "schema" if in_metadata => {}
                    "schemaversion" if in_metadata => {}
                    "organizations" => {
                        in_organizations = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"default" {
                                manifest.default_org =
                                    Some(String::from_utf8_lossy(&attr.value).to_string());
                            }
                        }
                    }
                    "item" if in_organizations => {
                        in_item = true;
                        current_item_id.clear();
                        current_item_title.clear();
                        current_item_ref.clear();
                        current_mastery = None;
                        current_prerequisites = None;

                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "identifier" => current_item_id = val,
                                "identifierref" => current_item_ref = val,
                                _ => {}
                            }
                        }
                    }
                    "title" if in_item => {
                        title_depth = depth;
                    }
                    "title" if in_organizations && !in_item => {
                        title_depth = depth;
                    }
                    "resources" => {
                        in_resources = true;
                    }
                    "resource" if in_resources => {
                        let mut res_id = String::new();
                        let mut res_href = String::new();
                        let mut res_type = String::new();

                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "identifier" => res_id = val,
                                "href" => res_href = val,
                                "type" | "adlcp:scormtype" | "adlcp:scormType" => res_type = val,
                                _ => {}
                            }
                        }

                        // Also check namespaced scormtype attributes
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            if key.contains("scormtype") || key.contains("scormType") {
                                res_type = String::from_utf8_lossy(&attr.value).to_lowercase();
                            }
                        }

                        if !res_id.is_empty() {
                            resources.insert(res_id, res_href);
                        }
                        let _ = res_type; // used below when building SCOs
                    }
                    _ => {}
                }

                current_text.clear();
            }
            Ok(Event::Text(ref e)) => {
                current_text = e.unescape().unwrap_or_default().to_string();
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();

                match name.as_str() {
                    "metadata" if depth == 2 => in_metadata = false,
                    "schema" if in_metadata => {
                        manifest.metadata.schema = Some(current_text.clone());
                        // Detect SCORM version from schema
                        if current_text.contains("2004") {
                            manifest.scorm_version = "2004".to_string();
                        }
                    }
                    "schemaversion" if in_metadata => {
                        manifest.metadata.schema_version = Some(current_text.clone());
                        if current_text.starts_with("2004") || current_text.contains("CAM") {
                            manifest.scorm_version = "2004".to_string();
                        }
                    }
                    "title" if depth == title_depth && in_item => {
                        current_item_title = current_text.clone();
                    }
                    "title" if depth == title_depth && in_organizations && !in_item => {
                        // Organization-level title = course title
                        if manifest.title.is_empty() {
                            manifest.title = current_text.clone();
                        }
                    }
                    "item" if in_organizations => {
                        if !current_item_ref.is_empty() {
                            pending_items.push(PendingItem {
                                identifier: current_item_id.clone(),
                                title: current_item_title.clone(),
                                identifierref: current_item_ref.clone(),
                                mastery_score: current_mastery,
                                prerequisites: current_prerequisites.clone(),
                            });
                        }
                        in_item = false;
                    }
                    "organizations" => in_organizations = false,
                    "resources" => in_resources = false,
                    _ => {}
                }

                depth -= 1;
            }
            Ok(Event::Empty(ref e)) => {
                // Self-closing tags like <resource ... />
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if name == "resource" && in_resources {
                    let mut res_id = String::new();
                    let mut res_href = String::new();

                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        match key.as_str() {
                            "identifier" => res_id = val,
                            "href" => res_href = val,
                            _ => {}
                        }
                    }

                    if !res_id.is_empty() {
                        resources.insert(res_id, res_href);
                    }
                }

                // Handle adlcp:masteryscore as empty element with value
                if name == "masteryscore" || name.ends_with(":masteryscore") {
                    if let Ok(score) = current_text.trim().parse::<f64>() {
                        current_mastery = Some(score);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    // Resolve pending items now that all resources are known
    for item in pending_items {
        let href = resources
            .get(&item.identifierref)
            .cloned()
            .unwrap_or_default();

        manifest.scos.push(ScormSco {
            identifier: item.identifier,
            title: item.title,
            href,
            sco_type: "sco".to_string(),
            mastery_score: item.mastery_score,
            prerequisites: item.prerequisites,
            max_time_allowed: None,
            time_limit_action: None,
        });
    }

    // Set launch URL to first SCO's href
    if let Some(first_sco) = manifest.scos.first() {
        manifest.launch_url = first_sco.href.clone();
    }

    // Fallback title
    if manifest.title.is_empty() {
        manifest.title = manifest
            .scos
            .first()
            .map(|s| s.title.clone())
            .unwrap_or_else(|| "Untitled Course".to_string());
    }

    Ok(manifest)
}

/// Parse a SCORM manifest from a ZIP archive.
///
/// Looks for `imsmanifest.xml` at the root of the ZIP file.
pub fn parse_from_zip<R: Read + std::io::Seek>(reader: R) -> Result<ScormManifest, String> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("Invalid ZIP: {e}"))?;

    let mut manifest_xml = archive
        .by_name("imsmanifest.xml")
        .map_err(|_| "imsmanifest.xml not found in SCORM package".to_string())?;

    let mut xml_bytes = Vec::new();
    manifest_xml
        .read_to_end(&mut xml_bytes)
        .map_err(|e| format!("Failed to read manifest: {e}"))?;

    parse_manifest(&xml_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scorm_12_manifest() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest identifier="com.example.course" version="1.0"
          xmlns="http://www.imsproject.org/xsd/imscp_rootv1p1p2"
          xmlns:adlcp="http://www.adlnet.org/xsd/adlcp_rootv1p2">
  <metadata>
    <schema>ADL SCORM</schema>
    <schemaversion>1.2</schemaversion>
  </metadata>
  <organizations default="org1">
    <organization identifier="org1">
      <title>Use of Force Training</title>
      <item identifier="item1" identifierref="res1">
        <title>Module 1: Legal Framework</title>
      </item>
      <item identifier="item2" identifierref="res2">
        <title>Module 2: De-escalation</title>
      </item>
    </organization>
  </organizations>
  <resources>
    <resource identifier="res1" type="webcontent" adlcp:scormtype="sco" href="module1/index.html"/>
    <resource identifier="res2" type="webcontent" adlcp:scormtype="sco" href="module2/index.html"/>
  </resources>
</manifest>"#;

        let manifest = parse_manifest(xml.as_bytes()).unwrap();

        assert_eq!(manifest.scorm_version, "1.2");
        assert_eq!(manifest.title, "Use of Force Training");
        assert_eq!(manifest.scos.len(), 2);
        assert_eq!(manifest.scos[0].title, "Module 1: Legal Framework");
        assert_eq!(manifest.scos[0].href, "module1/index.html");
        assert_eq!(manifest.scos[1].title, "Module 2: De-escalation");
        assert_eq!(manifest.launch_url, "module1/index.html");
    }

    #[test]
    fn parse_scorm_2004_manifest() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest identifier="course2004"
          xmlns="http://www.imsglobal.org/xsd/imscp_v1p1"
          xmlns:adlcp="http://www.adlnet.org/xsd/adlcp_v1p3">
  <metadata>
    <schema>ADL SCORM</schema>
    <schemaversion>2004 4th Edition</schemaversion>
  </metadata>
  <organizations default="default_org">
    <organization identifier="default_org">
      <title>Miranda Rights Refresher</title>
      <item identifier="item_01" identifierref="resource_01">
        <title>Miranda Warning Overview</title>
      </item>
    </organization>
  </organizations>
  <resources>
    <resource identifier="resource_01" type="webcontent" adlcp:scormType="sco" href="content/index.html"/>
  </resources>
</manifest>"#;

        let manifest = parse_manifest(xml.as_bytes()).unwrap();

        assert_eq!(manifest.scorm_version, "2004");
        assert_eq!(manifest.title, "Miranda Rights Refresher");
        assert_eq!(manifest.scos.len(), 1);
        assert_eq!(manifest.launch_url, "content/index.html");
    }
}
