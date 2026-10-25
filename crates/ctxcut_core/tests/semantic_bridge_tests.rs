//! Comprehensive tests for Stage 4: Semantic Bridge & Unity Analyzer.
//!
//! Tests:
//! 1. SemanticMetadataHeader generation and comment formatting.
//! 2. Zero GC allocation heuristic detection on pure C# methods.
//! 3. GC allocation detection (LINQ, `new ` class / array creations, boxing, coroutines).
//! 4. C# event declaration and invocation detection.

use ctxcut_core::framework::{
    analyze_allocations, build_semantic_header, detect_event_declarations,
    detect_event_invocations, detect_event_subscribers, EventMetadata, GcAllocReport,
    SemanticMetadataHeader, UnityAnalyzer,
};

#[test]
fn test_semantic_metadata_header_generation() {
    let header = SemanticMetadataHeader::new(
        vec![
            "CombatSystem.cs:42".to_string(),
            "Projectile.cs:88".to_string(),
        ],
        vec![EventMetadata::new(
            "OnHealthChanged",
            vec!["HealthBarView.cs".to_string()],
        )],
        GcAllocReport::zero_alloc(),
    );

    let comments = header.to_header_comments();
    let expected = "\
// [META] Callers: CombatSystem.cs:42, Projectile.cs:88\n\
// [META] Events emitted: OnHealthChanged (Subscribers: HealthBarView.cs)\n\
// [META] Allocations: 0 GC alloc detected in this method";

    assert_eq!(comments, expected);

    // Test prepending to method
    let method_code = "public void TakeDamage(int amount) { /* ... */ }";
    let prepended = header.prepend_to(method_code);
    assert!(prepended.starts_with(expected));
    assert!(prepended.ends_with(method_code));
}

#[test]
fn test_header_caller_truncation_to_five() {
    let callers = vec![
        "File1.cs:10".to_string(),
        "File2.cs:20".to_string(),
        "File3.cs:30".to_string(),
        "File4.cs:40".to_string(),
        "File5.cs:50".to_string(),
        "File6.cs:60".to_string(),
        "File7.cs:70".to_string(),
    ];

    let header = SemanticMetadataHeader::new(callers, Vec::new(), GcAllocReport::zero_alloc());

    let comments = header.to_header_comments();
    assert!(comments.contains("File1.cs:10, File2.cs:20, File3.cs:30, File4.cs:40, File5.cs:50"));
    assert!(!comments.contains("File6.cs:60"));
    assert!(!comments.contains("File7.cs:70"));
}

#[test]
fn test_zero_gc_allocation_detection_pure_csharp() {
    let pure_method = r#"
    public void CalculatePhysics(float deltaTime)
    {
        Vector3 velocity = new Vector3(1.0f, 0.0f, 0.0f);
        Vector3 position = new Vector3(0.0f, 0.0f, 0.0f);
        position += velocity * deltaTime;

        Color debugColor = new Color(1.0f, 0.0f, 0.0f, 1.0f);
        Quaternion rotation = new Quaternion(0.0f, 0.0f, 0.0f, 1.0f);
        int tickCount = new int();
        float speed = 5.5f;
        double precision = 12.345;
        bool isActive = true;
    }
    "#;

    let report = analyze_allocations(pure_method, Some("CalculatePhysics"));
    assert!(
        report.is_zero(),
        "Expected 0 GC alloc for primitive structs, got: {:?}",
        report.potential_allocs
    );
    assert_eq!(
        report.to_display_string(),
        "0 GC alloc detected in this method"
    );
}

#[test]
fn test_zero_gc_allocation_with_custom_struct() {
    let code_with_struct = r#"
    public struct CharacterStats
    {
        public int Health;
        public float Armor;
    }

    public void ApplyStats()
    {
        CharacterStats stats = new CharacterStats();
        stats.Health = 100;
        stats.Armor = 15.0f;
    }
    "#;

    let report = analyze_allocations(code_with_struct, Some("ApplyStats"));
    assert!(
        report.is_zero(),
        "Custom struct should be recognized as 0 GC alloc"
    );
    assert_eq!(
        report.to_display_string(),
        "0 GC alloc detected in this method"
    );
}

#[test]
fn test_gc_allocations_detection_linq_and_new_class() {
    let code_with_allocs = r#"
    public void ProcessEnemies(List<Enemy> enemies)
    {
        var alive = enemies.Where(e => e.IsAlive).Select(e => e.Score).ToList();
        var array = alive.ToArray();
        var ordered = enemies.OrderBy(e => e.Priority).GroupBy(e => e.Faction);
        bool hasBoss = enemies.Any(e => e.IsBoss);
        int total = enemies.Count(e => e.IsVisible);

        HeavyMonster monster = new HeavyMonster();
        List<int> numbers = new List<int>();
    }
    "#;

    let report = analyze_allocations(code_with_allocs, Some("ProcessEnemies"));
    assert!(!report.is_zero());

    let display = report.to_display_string();
    assert!(display.contains("Potential GC allocs:"));
    assert!(display.contains("'new HeavyMonster()'"));
    assert!(display.contains("'new List()'"));
    assert!(display.contains("LINQ '.Where()'"));
    assert!(display.contains("LINQ '.Select()'"));
    assert!(display.contains("LINQ '.ToList()'"));
    assert!(display.contains("LINQ '.ToArray()'"));
    assert!(display.contains("LINQ '.OrderBy()'"));
    assert!(display.contains("LINQ '.GroupBy()'"));
    assert!(display.contains("LINQ '.Any()'"));
    assert!(display.contains("LINQ '.Count()'"));
}

#[test]
fn test_gc_allocations_detection_arrays() {
    let array_code = r#"
    public void SpawnBuffers()
    {
        int[] buffer = new int[128];
        Vector3[] positions = new Vector3[16];
    }
    "#;

    let report = analyze_allocations(array_code, Some("SpawnBuffers"));
    assert!(
        !report.is_zero(),
        "Arrays in C# must be detected as GC allocations"
    );
    let display = report.to_display_string();
    assert!(display.contains("'new int[]'"));
    assert!(display.contains("'new Vector3[]'"));
}

#[test]
fn test_gc_allocations_boxing_and_coroutine_and_frame_strings() {
    let frame_code = r#"
    public void Update()
    {
        string label = "Score: " + score;
        string debug = $"Player at: {x}, {y}";
        string formatted = string.Format("Health: {0}", hp);
        object boxed = (object)42;
        StartCoroutine(FlashRoutine());
    }
    "#;

    let report = analyze_allocations(frame_code, Some("Update"));
    assert!(!report.is_zero());

    let display = report.to_display_string();
    assert!(display.contains("string concatenation '+' in frame method"));
    assert!(display.contains("string interpolation '$' in frame method"));
    assert!(display.contains("string formatting 'string.Format()' in frame method"));
    assert!(display.contains("boxing '(object)'"));
    assert!(display.contains("coroutine 'StartCoroutine'"));
}

#[test]
fn test_csharp_event_declarations_detection() {
    let event_declarations_code = r#"
    public class CombatManager : MonoBehaviour
    {
        public event Action OnCombatStarted;
        public event Action<int, float> OnDamageDealt;
        public event EventHandler OnReset;
        public event EventHandler<CustomEventArgs> OnStateChanged;
        public event UnityAction OnButtonClicked;
        public event UnityAction<bool> OnToggleChanged;
        public UnityEvent OnUnityCombatEvent;
        public UnityEvent<int> OnScoreUpdated;
    }
    "#;

    let events = detect_event_declarations(event_declarations_code);
    let names: Vec<&str> = events.iter().map(|e| e.name.as_str()).collect();

    assert!(names.contains(&"OnCombatStarted"));
    assert!(names.contains(&"OnDamageDealt"));
    assert!(names.contains(&"OnReset"));
    assert!(names.contains(&"OnStateChanged"));
    assert!(names.contains(&"OnButtonClicked"));
    assert!(names.contains(&"OnToggleChanged"));
    assert!(names.contains(&"OnUnityCombatEvent"));
    assert!(names.contains(&"OnScoreUpdated"));

    let damage_event = events
        .iter()
        .find(|e| e.name == "OnDamageDealt")
        .expect("Found OnDamageDealt");
    assert_eq!(damage_event.handler_type, "Action<int, float>");
}

#[test]
fn test_csharp_event_invocations_detection() {
    let invocation_code = r#"
    public void TakeDamage(int damage)
    {
        currentHealth -= damage;
        if (currentHealth <= 0)
        {
            OnHealthChanged?.Invoke();
            OnDied?.Invoke(playerId);
            OnCombatEnded.Invoke();
        }
    }
    "#;

    let invocations = detect_event_invocations(invocation_code);
    let names: Vec<&str> = invocations.iter().map(|i| i.event_name.as_str()).collect();

    assert!(names.contains(&"OnHealthChanged"));
    assert!(names.contains(&"OnDied"));
    assert!(names.contains(&"OnCombatEnded"));

    let health_inv = invocations
        .iter()
        .find(|i| i.event_name == "OnHealthChanged")
        .unwrap();
    assert_eq!(health_inv.expression, "OnHealthChanged?.Invoke()");

    let died_inv = invocations
        .iter()
        .find(|i| i.event_name == "OnDied")
        .unwrap();
    assert_eq!(died_inv.expression, "OnDied?.Invoke(playerId)");
}

#[test]
fn test_csharp_event_subscribers_detection() {
    let subscriber_code = r#"
    public class HealthBarView : MonoBehaviour
    {
        private void OnEnable()
        {
            combat.OnHealthChanged += HandleHealthChanged;
            restartButton.onClick.AddListener(OnRestart);
        }
    }
    "#;

    let subs = detect_event_subscribers(subscriber_code, "HealthBarView.cs");
    assert_eq!(subs.len(), 2);

    assert_eq!(subs[0].event_name, "OnHealthChanged");
    assert_eq!(subs[0].subscriber_file, "HealthBarView.cs");

    assert_eq!(subs[1].event_name, "onClick");
    assert_eq!(subs[1].subscriber_file, "HealthBarView.cs");
}

#[test]
fn test_build_semantic_header_complete_pipeline() {
    let method_code = r#"
    public void TakeDamage(int damage)
    {
        currentHealth -= damage;
        if (currentHealth <= 0)
        {
            OnHealthChanged?.Invoke();
        }
    }
    "#;

    let callers = vec![
        "CombatSystem.cs:42".to_string(),
        "Projectile.cs:88".to_string(),
    ];

    let subscribers = vec![(
        "OnHealthChanged".to_string(),
        "HealthBarView.cs".to_string(),
    )];

    let header = build_semantic_header(Some("TakeDamage"), method_code, callers, &subscribers);

    let comments = header.to_header_comments();
    let expected = "\
// [META] Callers: CombatSystem.cs:42, Projectile.cs:88\n\
// [META] Events emitted: OnHealthChanged (Subscribers: HealthBarView.cs)\n\
// [META] Allocations: 0 GC alloc detected in this method";

    assert_eq!(comments, expected);
}

#[test]
fn test_unity_analyzer_framework_matching() {
    use ctxcut_core::framework::FrameworkAnalyzer;
    use std::path::Path;

    let analyzer = UnityAnalyzer::new();
    assert_eq!(analyzer.name(), "unity");

    let cs_path = Path::new("Assets/Scripts/PlayerController.cs");
    let unity_code = "using UnityEngine;\npublic class PlayerController : MonoBehaviour { }";
    assert!(analyzer.matches_framework(cs_path, unity_code));

    let rust_path = Path::new("src/main.rs");
    assert!(!analyzer.matches_framework(rust_path, unity_code));

    let non_unity_cs = Path::new("Program.cs");
    let plain_csharp = "namespace ConsoleApp { class Program { static void Main() {} } }";
    assert!(!analyzer.matches_framework(non_unity_cs, plain_csharp));
}
