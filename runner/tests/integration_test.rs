// Integration test for runner crate
// Tests the integration with the math crate

#[test]
fn test_math_integration() {
    let result = math::add(5, 3);
    assert_eq!(result, 8);
}

#[test]
fn test_math_floating_point() {
    let result = math::add(2.5, 3.7);
    assert!((result - 6.2_f64).abs() < 0.0001);
}

#[test]
fn test_math_zero() {
    let result = math::add(0, 5);
    assert_eq!(result, 5);

    let result = math::add(5, 0);
    assert_eq!(result, 5);

    let result = math::add(0, 0);
    assert_eq!(result, 0);
}

#[test]
fn test_math_negative() {
    let result = math::add(-5, 3);
    assert_eq!(result, -2);

    let result = math::add(5, -3);
    assert_eq!(result, 2);

    let result = math::add(-5, -3);
    assert_eq!(result, -8);
}
