use api::ml::logreg::{LogRegModel, LogRegTrainConfig, train_logreg};

#[test]
fn logreg_learns_simple_or() {
    let x = vec![
        vec![0.0, 0.0],
        vec![0.0, 1.0],
        vec![1.0, 0.0],
        vec![1.0, 1.0],
    ];
    let y = vec![0.0, 1.0, 1.0, 1.0];

    let cfg = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 800,
        l2: 0.1,
    };

    let model = train_logreg(&x, &y, &cfg).expect("train");

    let p00 = model.predict_proba(&[0.0, 0.0]).expect("predict");
    let p11 = model.predict_proba(&[1.0, 1.0]).expect("predict");
    assert!(p00 < 0.5, "expected OR([0,0]) proba < 0.5, got {p00}");
    assert!(p11 > 0.5, "expected OR([1,1]) proba > 0.5, got {p11}");

    let json = serde_json::to_string(&model).expect("serialize");
    let model2: LogRegModel = serde_json::from_str(&json).expect("deserialize");
    let p11_2 = model2.predict_proba(&[1.0, 1.0]).expect("predict");
    assert!((p11_2 - p11).abs() < 1e-9);
}
