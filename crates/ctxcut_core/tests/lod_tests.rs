//! Unit tests for Stage 1: LOD Architecture & C# Skeleton Engine.

pub use ctxcut_core::lang::{slice_csharp_lod, CSharpLodEngine};
pub use ctxcut_core::slice::{
    ClassSkeleton, LodLevel, LodSliceResult, SkeletonField, SkeletonMethod,
    COLLAPSED_CONTEXT_COMMENT, MAX_HELPER_LINES, TARGET_SYMBOL_COMMENT, TRANSITIVE_HELPER_COMMENT,
};
pub use ctxcut_core::*;

// =========================================================================
// 1. TEST LOD 0: Signature & XML-Doc Extraction
// =========================================================================

#[test]
fn test_lod0_signature_and_xml_doc() {
    let source = r#"
namespace Game.Combat
{
    public class CombatCalculator
    {
        private float armorMultiplier = 0.85f;

        /// <summary>
        /// Calculates the physical damage reduction based on current armor value.
        /// </summary>
        /// <param name="rawDamage">The incoming unmitigated damage.</param>
        /// <returns>Mitigated damage integer value.</returns>
        public int CalculateDamage(int rawDamage)
        {
            var reduced = rawDamage * armorMultiplier;
            return (int)reduced;
        }

        private void ResetCache()
        {
            // cache reset logic
        }
    }
}
"#;

    let result = slice_csharp_lod(source, "CalculateDamage", LodLevel::Lod0)
        .expect("LOD 0 slice should succeed");

    assert_eq!(result.lod_level, LodLevel::Lod0);
    assert_eq!(result.target_symbol, "CalculateDamage");
    assert_eq!(result.class_name, Some("CombatCalculator".to_string()));

    // 1. Must contain XML-doc comments
    assert!(
        result.content.contains("/// <summary>"),
        "Content must contain XML-doc summary: {}",
        result.content
    );
    assert!(
        result
            .content
            .contains("Calculates the physical damage reduction"),
        "Content must contain doc description"
    );
    assert!(
        result.content.contains("/// <param name=\"rawDamage\">"),
        "Content must contain parameter doc"
    );

    // 2. Must contain signature
    assert!(
        result
            .content
            .contains("public int CalculateDamage(int rawDamage)"),
        "Content must contain method signature: {}",
        result.content
    );

    // 3. Must NOT contain method body implementation
    assert!(
        !result.content.contains("rawDamage * armorMultiplier"),
        "LOD 0 must NOT contain method body implementation"
    );
    assert!(
        !result.content.contains("return (int)reduced"),
        "LOD 0 must NOT contain return statement"
    );

    // 4. Must NOT contain class fields or other sibling methods
    assert!(
        !result.content.contains("armorMultiplier = 0.85f"),
        "LOD 0 must NOT contain class fields"
    );
    assert!(
        !result.content.contains("ResetCache"),
        "LOD 0 must NOT contain sibling methods"
    );

    assert!(result.expanded_helpers.is_empty());
    assert!(result.collapsed_methods.is_empty());
}

// =========================================================================
// 2. TEST LOD 1: PlayerHealth MonoBehaviour Skeleton + Focus
// =========================================================================

#[test]
fn test_lod1_monobehaviour_skeleton_and_target_body() {
    let source = r#"
using UnityEngine;
using System;

namespace Game.Player
{
    public class PlayerHealth : MonoBehaviour, IDamageable
    {
        [Header("Health Settings")]
        [SerializeField] private int maxHealth = 100;

        [SerializeField]
        private float invulnerabilityDuration = 1.5f;

        private int currentHealth;
        public bool isDead;

        private void Awake()
        {
            currentHealth = maxHealth;
            isDead = false;
        }

        private void OnDisable()
        {
            // Clean up event listeners and audio sources
        }

        public void TakeDamage(int damage)
        {
            if (isDead) return;
            currentHealth -= damage;
            if (currentHealth <= 0)
            {
                Die();
            }
        }

        private void Die()
        {
            isDead = true;
        }
    }
}
"#;

    let result = CSharpLodEngine::slice_source(source, "PlayerHealth.TakeDamage", LodLevel::Lod1)
        .expect("LOD 1 slice should succeed");

    assert_eq!(result.lod_level, LodLevel::Lod1);
    assert_eq!(result.target_symbol, "TakeDamage");
    assert_eq!(result.class_name, Some("PlayerHealth".to_string()));

    // 1. Class header must be visible
    assert!(
        result
            .content
            .contains("public class PlayerHealth : MonoBehaviour, IDamageable"),
        "Class declaration header must be visible: {}",
        result.content
    );

    // 2. All fields must be visible (especially [SerializeField], [Header], private/public)
    assert!(
        result.content.contains("[Header(\"Health Settings\")]"),
        "Header attribute on field must be visible"
    );
    assert!(
        result.content.contains("[SerializeField]"),
        "SerializeField attribute on fields must be visible"
    );
    assert!(
        result.content.contains("private int maxHealth = 100;"),
        "maxHealth field must be visible"
    );
    assert!(
        result.content.contains("invulnerabilityDuration = 1.5f;"),
        "invulnerabilityDuration field must be visible"
    );
    assert!(
        result.content.contains("private int currentHealth;"),
        "currentHealth private field must be visible"
    );
    assert!(
        result.content.contains("public bool isDead;"),
        "isDead public field must be visible"
    );

    // 3. Sibling methods collapsed into { ... } with comment [LOD 1: Collapsed context]
    assert!(
        result.content.contains(COLLAPSED_CONTEXT_COMMENT),
        "Collapsed comment must be present above collapsed sibling methods"
    );
    assert!(
        result.content.contains("private void Awake() { ... }"),
        "Awake() method must be collapsed: {}",
        result.content
    );
    assert!(
        result.content.contains("private void OnDisable() { ... }"),
        "OnDisable() method must be collapsed: {}",
        result.content
    );
    assert!(
        result.content.contains("private void Die() { ... }"),
        "Die() method must be collapsed in LOD 1: {}",
        result.content
    );

    // Sibling bodies must NOT be in content
    assert!(
        !result.content.contains("currentHealth = maxHealth;"),
        "Awake body must NOT be in LOD 1 content"
    );
    assert!(
        !result.content.contains("Clean up event listeners"),
        "OnDisable body must NOT be in LOD 1 content"
    );

    // 4. Target method has [TARGET SYMBOL: FULL BODY] and full verbatim body
    assert!(
        result.content.contains(TARGET_SYMBOL_COMMENT),
        "Target symbol comment must be placed above target method"
    );
    assert!(
        result
            .content
            .contains("public void TakeDamage(int damage)"),
        "Target method signature must be present"
    );
    assert!(
        result.content.contains("currentHealth -= damage;"),
        "Target method full body must be present: {}",
        result.content
    );
    assert!(
        result.content.contains("if (currentHealth <= 0)"),
        "Target method inner logic must be preserved verbatim"
    );

    // Check reported collapsed methods
    assert!(result.collapsed_methods.contains(&"Awake".to_string()));
    assert!(result.collapsed_methods.contains(&"OnDisable".to_string()));
    assert!(result.collapsed_methods.contains(&"Die".to_string()));
    assert!(result.expanded_helpers.is_empty());
}

// =========================================================================
// 3. TEST LOD 2: Transitive Slice (Helpers <= 15 LOC vs > 15 LOC)
// =========================================================================

#[test]
fn test_lod2_transitive_slice_short_and_long_helpers() {
    assert_eq!(MAX_HELPER_LINES, 15);
    assert_eq!(TRANSITIVE_HELPER_COMMENT, "// [LOD 2: Transitive helper]");

    let source = r#"
using System;

public class BossController
{
    private int phase = 1;
    private int rageMeter = 0;

    private void Start()
    {
        phase = 1;
    }

    public void ExecutePhaseTransition(int nextPhase)
    {
        ValidatePhase(nextPhase);
        phase = nextPhase;
        PlayPhaseTransitionCinematic();
    }

    // Direct private helper <= 15 lines of code (7 lines total, 5 lines body)
    private void ValidatePhase(int nextPhase)
    {
        if (nextPhase <= phase)
        {
            throw new ArgumentException("Invalid next phase");
        }
    }

    // Direct private helper > 15 lines of code (22 lines)
    private void PlayPhaseTransitionCinematic()
    {
        // Step 1: Camera shake
        // Step 2: Roar animation
        // Step 3: Spawn minions
        // Step 4: Dark sky shader
        // Step 5: Screen flash
        // Step 6: Audio cue
        // Step 7: Invulnerability bubble
        // Step 8: Particle eruption
        // Step 9: UI banner update
        // Step 10: Reset player targets
        // Step 11: Shockwave pulse
        // Step 12: Ground fissure
        // Step 13: Lightning strike
        // Step 14: Health bar refactor
        // Step 15: Boss dialogue trigger
        // Step 16: Cinematic complete
    }
}
"#;

    let result = slice_csharp_lod(source, "ExecutePhaseTransition", LodLevel::Lod2)
        .expect("LOD 2 slice should succeed");

    assert_eq!(result.lod_level, LodLevel::Lod2);
    assert_eq!(result.target_symbol, "ExecutePhaseTransition");
    assert_eq!(result.class_name, Some("BossController".to_string()));

    // 1. Target method has TARGET_SYMBOL_COMMENT and full body
    assert!(
        result.content.contains(TARGET_SYMBOL_COMMENT),
        "Must have target symbol comment"
    );
    assert!(
        result.content.contains("ValidatePhase(nextPhase);"),
        "Target method body must be visible"
    );
    assert!(
        result.content.contains("PlayPhaseTransitionCinematic();"),
        "Target method body invocation must be visible"
    );

    // 2. Short private helper (ValidatePhase, <= 15 lines):
    // Body MUST be expanded!
    assert!(
        result
            .expanded_helpers
            .contains(&"ValidatePhase".to_string()),
        "ValidatePhase must be in expanded_helpers: {:?}",
        result.expanded_helpers
    );
    assert!(
        result
            .content
            .contains("throw new ArgumentException(\"Invalid next phase\");"),
        "ValidatePhase body must be expanded in LOD 2: {}",
        result.content
    );
    assert!(
        result.content.contains(TRANSITIVE_HELPER_COMMENT),
        "Transitive helper comment must precede expanded helper"
    );
    assert!(
        !result
            .content
            .contains("private void ValidatePhase(int nextPhase) { ... }"),
        "ValidatePhase must NOT be collapsed into {{ ... }}"
    );

    // 3. Long private helper (PlayPhaseTransitionCinematic, > 15 lines):
    // Body MUST be collapsed into { ... }!
    assert!(
        result
            .collapsed_methods
            .contains(&"PlayPhaseTransitionCinematic".to_string()),
        "PlayPhaseTransitionCinematic must be in collapsed_methods: {:?}",
        result.collapsed_methods
    );
    assert!(
        result
            .content
            .contains("private void PlayPhaseTransitionCinematic() { ... }"),
        "PlayPhaseTransitionCinematic must be collapsed into {{ ... }}: {}",
        result.content
    );
    assert!(
        !result.content.contains("Step 10: Reset player targets"),
        "PlayPhaseTransitionCinematic body must NOT be in content"
    );

    // 4. Non-called sibling method (Start):
    // Must be collapsed with [LOD 1: Collapsed context]
    assert!(
        result.collapsed_methods.contains(&"Start".to_string()),
        "Start must be in collapsed_methods"
    );
    assert!(
        result.content.contains("private void Start() { ... }"),
        "Start must be collapsed into {{ ... }}"
    );
}

// =========================================================================
// 4. TEST SERDE & ENUM SERIALIZATION
// =========================================================================

#[test]
fn test_lod_level_serde_and_parsing() {
    // Serialization
    assert_eq!(serde_json::to_string(&LodLevel::Lod0).unwrap(), "\"lod0\"");
    assert_eq!(serde_json::to_string(&LodLevel::Lod1).unwrap(), "\"lod1\"");
    assert_eq!(serde_json::to_string(&LodLevel::Lod2).unwrap(), "\"lod2\"");

    // Deserialization aliases
    let d0: LodLevel = serde_json::from_str("\"lod0\"").unwrap();
    assert_eq!(d0, LodLevel::Lod0);
    let d0_alias: LodLevel = serde_json::from_str("\"signature\"").unwrap();
    assert_eq!(d0_alias, LodLevel::Lod0);
    let d0_num: LodLevel = serde_json::from_str("\"0\"").unwrap();
    assert_eq!(d0_num, LodLevel::Lod0);

    let d1: LodLevel = serde_json::from_str("\"lod1\"").unwrap();
    assert_eq!(d1, LodLevel::Lod1);
    let d1_alias: LodLevel = serde_json::from_str("\"skeleton\"").unwrap();
    assert_eq!(d1_alias, LodLevel::Lod1);

    let d2: LodLevel = serde_json::from_str("\"lod2\"").unwrap();
    assert_eq!(d2, LodLevel::Lod2);
    let d2_alias: LodLevel = serde_json::from_str("\"transitive\"").unwrap();
    assert_eq!(d2_alias, LodLevel::Lod2);

    // FromStr & Display
    assert_eq!("lod0".parse::<LodLevel>().unwrap(), LodLevel::Lod0);
    assert_eq!("skeleton".parse::<LodLevel>().unwrap(), LodLevel::Lod1);
    assert_eq!(
        "transitive_slice".parse::<LodLevel>().unwrap(),
        LodLevel::Lod2
    );
    assert_eq!(LodLevel::Lod1.to_string(), "lod1");

    // Full result serde roundtrip
    let res = LodSliceResult {
        lod_level: LodLevel::Lod2,
        target_symbol: "TakeDamage".to_string(),
        class_name: Some("PlayerHealth".to_string()),
        content: "public class PlayerHealth { ... }".to_string(),
        expanded_helpers: vec!["CheckDeath".to_string()],
        collapsed_methods: vec!["Awake".to_string(), "OnDisable".to_string()],
    };
    let json = serde_json::to_string(&res).unwrap();
    let deserialized: LodSliceResult = serde_json::from_str(&json).unwrap();
    assert_eq!(res, deserialized);
}

// =========================================================================
// 5. TEST EDGE CASES: Non-private Helpers and Direct Invocation Matching
// =========================================================================

#[test]
fn test_public_methods_are_not_expanded_as_private_helpers_in_lod2() {
    let source = r#"
public class TestService
{
    public void Process()
    {
        LogPublic();
        DoPrivate();
    }

    public void LogPublic()
    {
        Console.WriteLine("Public log");
    }

    private void DoPrivate()
    {
        Console.WriteLine("Private work");
    }
}
"#;

    let result =
        slice_csharp_lod(source, "Process", LodLevel::Lod2).expect("LOD 2 slice should succeed");

    // DoPrivate is private and short -> expanded
    assert!(result.expanded_helpers.contains(&"DoPrivate".to_string()));
    assert!(result
        .content
        .contains("Console.WriteLine(\"Private work\");"));

    // LogPublic is public -> must NOT be expanded in expanded_helpers
    assert!(!result.expanded_helpers.contains(&"LogPublic".to_string()));
    assert!(result.collapsed_methods.contains(&"LogPublic".to_string()));
    assert!(result.content.contains("public void LogPublic() { ... }"));
}

#[test]
fn test_missing_symbol_returns_error() {
    let source = r#"
public class Foo
{
    private void Bar() {}
}
"#;

    let err = slice_csharp_lod(source, "NonExistentMethod", LodLevel::Lod1);
    assert!(err.is_err(), "Non-existent symbol query must return error");
}
