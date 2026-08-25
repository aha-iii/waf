//! Tests extracted from `users/users.rs`.

use crate::users::*;

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("secret").expect("hash");
        assert!(verify_password("secret", &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn normalize_roles() {
        assert_eq!(normalize_role("Admin").as_deref(), Some("admin"));
        assert_eq!(normalize_role("viewer").as_deref(), Some("viewer"));
        assert!(normalize_role("guest").is_none());
    }
