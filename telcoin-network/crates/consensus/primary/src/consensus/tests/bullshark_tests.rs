//! Bullshark tests

use super::*;
use crate::{
    consensus::{consensus_utils::NUM_SUB_DAGS_PER_SCHEDULE, Consensus},
    test_utils::{
        make_certificates_with_epoch, make_certificates_with_leader_configuration,
        make_certificates_with_slow_nodes, make_optimal_certificates, mock_certificate,
        mock_certificate_with_epoch, temp_dir, TestLeaderConfiguration, TestLeaderSupport,
    },
    ConsensusBus,
};
use std::collections::{BTreeSet, HashMap};
use tn_config::{ConsensusConfig, NetworkConfig};
use tn_storage::{mem_db::MemDatabase, open_db, CertificateStore};
use tn_test_utils::CommitteeFixture;
use tn_types::{
    AuthorityIdentifier, ExecHeader, Notifier, SealedHeader, TaskManager, TnReceiver, TnSender,
    B256, DEFAULT_BAD_NODES_STAKE_THRESHOLD,
};
use tracing::info;

#[tokio::test]
async fn order_leaders() {
    // GIVEN
    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();
    // Make certificates for rounds 1 to 7.
    let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();
    let (certificates, _next_parents) =
        make_optimal_certificates(&committee, 1..=7, &genesis, &ids);

    let metrics = Arc::new(ConsensusMetrics::default());
    let gc_depth = 50;
    let mut state = ConsensusState::new(metrics.clone(), gc_depth);

    for certificate in certificates {
        state.try_insert(&certificate).unwrap();
    }

    let db = temp_dir();
    let store = open_db(db.path());
    let schedule = LeaderSchedule::new(committee.clone(), LeaderSwapTable::default());
    let bullshark = Bullshark::new(
        committee,
        store,
        metrics,
        NUM_SUB_DAGS_PER_SCHEDULE,
        schedule.clone(),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    // AND the leader of round 6
    let (_, leader) = schedule.leader_certificate(6, &state.dag);

    // WHEN
    let mut ordered_leaders = bullshark.order_leaders(leader.unwrap(), &state);

    // THEN
    // we expect all the leaders to be returned in round ascending order
    let mut expected_leader_rounds: VecDeque<Round> = VecDeque::from(vec![2, 4, 6]);
    while let Some(leader) = ordered_leaders.pop_front() {
        assert_eq!(leader.round(), expected_leader_rounds.pop_front().unwrap());
    }

    // we expect to have ordered all the 3 leaders
    assert!(expected_leader_rounds.is_empty());
}

#[tokio::test]
async fn commit_one_with_leader_schedule_change() {
    struct TestCase {
        description: String,
        rounds: Round,
        _expected_leaders: VecDeque<AuthorityIdentifier>,
    }

    let test_cases: Vec<TestCase> = vec![
        TestCase {
            description: "When schedule change is enabled, then authority 0 is bad node and swapped with authority 3".to_string(),
            rounds: 11,
            _expected_leaders: VecDeque::from(vec![
                AuthorityIdentifier::dummy_for_test(0),
                AuthorityIdentifier::dummy_for_test(1),
                AuthorityIdentifier::dummy_for_test(2),
                AuthorityIdentifier::dummy_for_test(3),
                AuthorityIdentifier::dummy_for_test(3),
            ]),
        },
    ];

    for test_case in test_cases {
        tracing::debug!("Running test case \"{}\"", test_case.description);
        // GIVEN
        let fixture = CommitteeFixture::builder(MemDatabase::default).build();
        let committee = fixture.committee();
        // Make certificates for rounds 1 to 9.
        let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
        let mut expected_leaders: VecDeque<AuthorityIdentifier> = ids.iter().cloned().collect();
        expected_leaders.push_back(ids.last().unwrap().clone());
        let genesis =
            Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();
        let (certificates, _next_parents) =
            make_optimal_certificates(&committee, 1..=test_case.rounds, &genesis, &ids);

        let metrics = Arc::new(ConsensusMetrics::default());
        let gc_depth = 50;
        let sub_dags_per_schedule = 3;
        let mut state = ConsensusState::new(metrics.clone(), gc_depth);
        let db = temp_dir();
        let store = open_db(db.path());
        let schedule = LeaderSchedule::new(committee.clone(), LeaderSwapTable::default());
        let bad_nodes_stake_threshold = 33;
        let mut bullshark = Bullshark::new(
            committee,
            store,
            metrics,
            sub_dags_per_schedule,
            schedule.clone(),
            bad_nodes_stake_threshold,
        );

        let mut committed_sub_dags = Vec::new();
        for certificate in certificates {
            let (outcome, committed) =
                bullshark.process_certificate(&mut state, certificate).unwrap();

            if outcome == Outcome::Commit {
                for sub_dag in &committed {
                    assert_eq!(sub_dag.leader.origin(), &expected_leaders.pop_front().unwrap())
                }
            }

            committed_sub_dags.extend(committed);
        }

        assert!(expected_leaders.is_empty());
    }
}

/// We test the scenario where a leader is recursively committed and that changes the schedule
/// because of new reputation scores. More specifically, the leaders of rounds 6, 8 & 10 either do
/// not receive enough support or they do not get referenced at all by the next round, essentially
/// postponing the commit that changes the schedule. Once finally a commit happens for round 13 that
/// recursively commits the leader of round 6, the schedule gets updated and the new leaders are
/// committed correctly.
#[tokio::test]
async fn not_enough_support_with_leader_schedule_change() {
    // GIVEN
    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();

    let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();

    let mut certificates: VecDeque<Certificate> = VecDeque::new();
    let mut leader_configs = HashMap::new();

    // The leader of round 6 should receive weak support from the certificates of round 7 so we
    // don't commit this leader immediately. That will basically postpone the update of the
    // leader schedule. We expect this leader to get committed through its connection with a
    // later leader.
    leader_configs.insert(
        6,
        TestLeaderConfiguration {
            round: 6,
            authority: ids.get(2).unwrap().clone(),
            should_omit: false,
            support: Some(TestLeaderSupport::Weak),
        },
    );

    // The leader of round 8 (with the old schedule) is receiving no support at all from any
    // of the children of round 9. So again, the leader schedule update will get postponed. As no
    // certificate of round 9 refers to this leader, we don't expect to get committed at all.
    leader_configs.insert(
        8,
        TestLeaderConfiguration {
            round: 8,
            authority: ids.get(3).unwrap().clone(),
            should_omit: false,
            support: Some(TestLeaderSupport::NoSupport),
        },
    );

    // We expect the leader of round 10 to be the authority with id 0 (reminder: a round robin
    // schedule is used for testing) with the "old" schedule. We construct the DAG with weak
    // support for this leader so we don't allow it to get committed immediately from the
    // trigger round 11. However, once the schedule gets updated, the authority with id 0 is
    // expected to be flagged as low score and will be replaced by the authority with id 3. Now,
    // since a recursive commit will take place the leader for round 10 will be the authority
    // with id 3, for which we will have a certificate present, thus we should observe the leader
    // get committed.
    leader_configs.insert(
        10,
        TestLeaderConfiguration {
            round: 10,
            authority: ids.first().unwrap().clone(),
            should_omit: false,
            support: Some(TestLeaderSupport::Weak),
        },
    );

    let (out, _parents) = make_certificates_with_leader_configuration(
        &committee,
        1..=15,
        &genesis,
        &ids,
        leader_configs,
    );
    certificates.extend(out);

    let metrics = Arc::new(ConsensusMetrics::default());
    let gc_depth = 50;
    let sub_dags_per_schedule = 4;
    let mut state = ConsensusState::new(metrics.clone(), gc_depth);
    let db = temp_dir();
    let store = open_db(db.path());
    let schedule = LeaderSchedule::new(committee.clone(), LeaderSwapTable::default());

    let bad_nodes_stake_threshold = 33;
    let mut bullshark = Bullshark::new(
        committee,
        store,
        metrics,
        sub_dags_per_schedule,
        schedule,
        bad_nodes_stake_threshold,
    );

    let mut total_13_certs = 0;
    let mut total_15_certs = 0;
    for certificate in certificates {
        let (outcome, committed) =
            bullshark.process_certificate(&mut state, certificate.clone()).unwrap();

        // on the round 7, 9 or 11 we should not commit the leader of previous round as there is not
        // enough support
        if certificate.round() == 7 || certificate.round() == 9 || certificate.round() == 11 {
            assert_eq!(outcome, Outcome::NotEnoughSupportForLeader);
        }

        // on round 13 we should commit the leader of round 6 and round 10.
        // Leader 8 should not exist amongst the committed ones.
        if certificate.round() == 13 {
            total_13_certs += 1;

            if total_13_certs == 2 {
                assert_eq!(committed.len(), 3);

                let committed_dag_6 = &committed[0];
                let committed_dag_10 = &committed[1];
                let committed_dag_12 = &committed[2];

                assert_eq!(committed_dag_6.leader_round(), 6);
                assert_eq!(committed_dag_10.leader_round(), 10);
                assert_eq!(committed_dag_12.leader_round(), 12);

                // Originally, as we do round robin the leaders in testing, we would expect the
                // leader of round 10 to be the Authority 0. However, since a reputation scores
                // update happened the leader schedule changed and now the Authority
                // 0 is flagged as low score and it will be swapped with Authority
                // 3.
                assert_eq!(committed_dag_10.leader.origin(), ids.get(3).unwrap());

                assert_eq!(outcome, Outcome::Commit);
            }
        }

        // on round 15 we should commit the leader of round 14
        if certificate.round() == 15 {
            total_15_certs += 1;

            if total_15_certs == 2 {
                assert_eq!(committed.len(), 1);

                let committed_dag_14 = &committed[0];

                assert_eq!(committed_dag_14.leader_round(), 14);

                assert_eq!(committed_dag_14.leader.origin(), ids.get(2).unwrap());
            }
        }
    }

    assert_eq!(total_13_certs, 4);
    assert_eq!(total_15_certs, 4);
}

/// We test here the leader schedule change when we experience a long period of asynchrony. That
/// prohibit us from committing for 8 rounds where 2 schedule changes should have happened given our
/// setup. Then once we manage to commit on round 15 we observe 2 schedule changes happening and 5
/// commits.
#[tokio::test]
async fn test_long_period_of_asynchrony_for_leader_schedule_change() {
    // GIVEN
    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();

    let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();

    let mut certificates = VecDeque::new();
    let mut leader_configs = HashMap::new();

    // A vector of tuples (leader_round, leader_authority_id)
    let leaders_with_weak_support = vec![(6, 2), (8, 3), (10, 0), (12, 1)];

    // We make the leaders for the corresponding rounds receive weak support, so we can't commit
    // immediately
    for (round, authority_id) in leaders_with_weak_support {
        leader_configs.insert(
            round,
            TestLeaderConfiguration {
                round,
                authority: ids.get(authority_id).unwrap().clone(),
                should_omit: false,
                support: Some(TestLeaderSupport::Weak),
            },
        );
    }

    let (out, _parents) = make_certificates_with_leader_configuration(
        &committee,
        1..=15,
        &genesis,
        &ids,
        leader_configs,
    );
    certificates.extend(out);

    let metrics = Arc::new(ConsensusMetrics::default());
    let gc_depth = 50;
    let sub_dags_per_schedule = 4;
    let mut state = ConsensusState::new(metrics.clone(), gc_depth);
    let db = temp_dir();
    let store = open_db(db.path());
    let schedule = LeaderSchedule::new(committee.clone(), LeaderSwapTable::default());

    let bad_nodes_stake_threshold = 33;
    let mut bullshark = Bullshark::new(
        committee.clone(),
        store,
        metrics,
        sub_dags_per_schedule,
        schedule,
        bad_nodes_stake_threshold,
    );

    let mut total = 0;
    for certificate in certificates {
        let (outcome, committed) =
            bullshark.process_certificate(&mut state, certificate.clone()).unwrap();

        if certificate.round() == 7
            || certificate.round() == 9
            || certificate.round() == 11
            || certificate.round() == 13
        {
            assert_eq!(outcome, Outcome::NotEnoughSupportForLeader);
        }

        if certificate.round() == 15 {
            total += 1;

            if total == 2 {
                assert_eq!(committed.len(), 5);

                let committed_dag_6 = &committed[0];
                let committed_dag_8 = &committed[1];
                let committed_dag_10 = &committed[2];
                let committed_dag_12 = &committed[3];
                let committed_dag_14 = &committed[4];

                assert_eq!(committed_dag_6.leader_round(), 6);
                assert_eq!(committed_dag_8.leader_round(), 8);
                assert_eq!(committed_dag_10.leader_round(), 10);
                assert_eq!(committed_dag_12.leader_round(), 12);
                assert_eq!(committed_dag_14.leader_round(), 14);

                // Two schedule changes have happened during this commit
                assert!(committed_dag_6.reputation_score.final_of_schedule);
                assert!(committed_dag_14.reputation_score.final_of_schedule);

                // Originally, as we do round robin the leaders in testing, we would expect the
                // leader of round 10 to be the Authority 0. However, since a reputation scores
                // update happened the leader schedule changed and now the Authority
                // 0 is flagged as low score and it will be swapped with Authority
                // 3.
                assert_eq!(committed_dag_10.leader.origin(), ids.get(3).unwrap());

                // The leaders of round 12 & 14 shouldn't change from the "original" schedule
                let schedule = LeaderSchedule::new(committee, LeaderSwapTable::default());

                assert_eq!(committed_dag_12.leader.origin(), &schedule.leader(12).id());
                assert_eq!(committed_dag_14.leader.origin(), &schedule.leader(14).id());

                assert_eq!(outcome, Outcome::Commit);

                break;
            }
        }
    }

    // ensure that we actually reached the point of processing two certificates of round 15.
    assert_eq!(total, 2);
}

/// Run for 4 dag rounds in ideal conditions (all nodes reference all other nodes). We should commit
/// the leader of round 2.
#[tokio::test]
async fn commit_one() {
    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();
    // Make certificates for rounds 1 and 2.
    let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();
    let (mut certificates, next_parents) =
        make_optimal_certificates(&committee, 1..=2, &genesis, &ids);

    // Make two certificate (f+1) with round 3 to trigger the commits.
    let (_, certificate) =
        mock_certificate(&committee, ids.first().unwrap().clone(), 3, next_parents.clone());
    certificates.push_back(certificate);
    let (_, certificate) =
        mock_certificate(&committee, ids.get(1).unwrap().clone(), 3, next_parents);
    certificates.push_back(certificate);

    let config = fixture.authorities().next().unwrap().consensus_config();
    let store = config.node_storage().clone();
    let metrics = Arc::new(ConsensusMetrics::default());

    let bullshark = Bullshark::new(
        committee.clone(),
        store.clone(),
        metrics.clone(),
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee.clone(), LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    let cb = ConsensusBus::new();
    let mut rx_output = cb.sequence().subscribe();
    Consensus::spawn(config, &cb, bullshark, &TaskManager::default());
    let cb_clone = cb.clone();
    let dummy_parent = SealedHeader::new(ExecHeader::default(), B256::default());
    cb.recent_blocks().send_modify(|blocks| blocks.push_latest(dummy_parent));
    tokio::spawn(async move {
        let mut rx_primary = cb_clone.committed_certificates().subscribe();
        while rx_primary.recv().await.is_some() {}
    });

    // Feed all certificates to the consensus. Only the last certificate should trigger
    // commits, so the task should not block.
    while let Some(certificate) = certificates.pop_front() {
        cb.new_certificates().send(certificate).await.unwrap();
    }

    // Ensure the first 4 ordered certificates are from round 1 (they are the parents of the
    // committed leader); then the leader's certificate should be committed.
    let committed_sub_dag: CommittedSubDag = rx_output.recv().await.unwrap();
    let mut sequence = committed_sub_dag.certificates.into_iter();
    for _ in 1..=4 {
        let output = sequence.next().unwrap();
        assert_eq!(output.round(), 1);
    }
    let output = sequence.next().unwrap();
    assert_eq!(output.round(), 2);

    // AND the reputation scores have not been updated
    assert_eq!(committed_sub_dag.reputation_score.total_authorities(), 4);
    assert!(committed_sub_dag.reputation_score.all_zero());
}

/// Run for 11 dag rounds with one dead node node (that is not a leader). We should commit the
/// leaders of rounds 2, 4, 6 and 10. The leader of round 8 will be missing, but eventually the
/// leader 10 will get committed.
#[tokio::test]
async fn dead_node() {
    // Make the certificates.
    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee: Committee = fixture.committee();
    let mut ids: Vec<_> = committee.authorities().iter().map(|authority| authority.id()).collect();

    // remove the last authority - 4
    let dead_node = ids.pop().unwrap();

    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();

    let (mut certificates, _) = make_optimal_certificates(&committee, 1..=11, &genesis, &ids);

    let config = fixture.authorities().next().unwrap().consensus_config();
    let store = config.node_storage().clone();
    let metrics = Arc::new(ConsensusMetrics::default());

    let bullshark = Bullshark::new(
        committee.clone(),
        store.clone(),
        metrics.clone(),
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee.clone(), LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    let cb = ConsensusBus::new();
    let dummy_parent = SealedHeader::new(ExecHeader::default(), B256::default());
    cb.recent_blocks().send_modify(|blocks| blocks.push_latest(dummy_parent));
    let mut rx_output = cb.sequence().subscribe();
    Consensus::spawn(config, &cb, bullshark, &TaskManager::default());
    let cb_clone = cb.clone();
    tokio::spawn(async move {
        let mut rx_primary = cb_clone.committed_certificates().subscribe();
        while rx_primary.recv().await.is_some() {}
    });

    let cb_clone = cb.clone();
    // Feed all certificates to the consensus.
    tokio::spawn(async move {
        while let Some(certificate) = certificates.pop_front() {
            cb_clone.new_certificates().send(certificate).await.unwrap();
        }
    });

    // We should commit 3 leaders (rounds 2, 4 and 6).
    let mut committed = Vec::new();
    let mut committed_sub_dags: Vec<CommittedSubDag> = Vec::new();
    for _commit_rounds in 1..=4 {
        let committed_sub_dag = rx_output.recv().await.unwrap();
        committed.extend(committed_sub_dag.certificates.clone());
        committed_sub_dags.push(committed_sub_dag);
    }

    let mut sequence = committed.into_iter();
    for i in 1..=27 {
        let output = sequence.next().unwrap();
        let expected = ((i - 1) / ids.len() as u32) + 1;
        assert_eq!(output.round(), expected);
    }
    let output = sequence.next().unwrap();
    assert_eq!(output.round(), 10);

    // AND check that the consensus scores are the expected ones
    for (index, sub_dag) in committed_sub_dags.iter().enumerate() {
        assert_eq!(sub_dag.reputation_score.total_authorities(), 4);

        // For the first commit we expect to have any only zero scores
        if index == 0 {
            sub_dag.reputation_score.scores_per_authority.iter().for_each(|(_key, score)| {
                assert_eq!(*score, 0_u64);
            });
        } else {
            // For any other commit we expect to always have a +1 score for each authority, as
            // everyone always votes for the leader
            for (key, score) in &sub_dag.reputation_score.scores_per_authority {
                if key == &dead_node {
                    assert_eq!(*score as usize, 0);
                } else {
                    assert_eq!(*score as usize, index);
                }
            }
        }
    }
}

/// Run for 5 dag rounds. The leader of round 2 does not have enough support, but the leader of
/// round 4 does. The leader of rounds 2 and 4 should thus be committed (because they are linked).
#[tokio::test]
async fn not_enough_support() {
    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();
    let mut ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    ids.sort();

    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();

    let mut certificates = VecDeque::new();

    // Round 1: Fully connected graph.
    let nodes: Vec<_> = ids.iter().take(3).cloned().collect();
    let (out, parents) = make_optimal_certificates(&committee, 1..=1, &genesis, &nodes);
    certificates.extend(out);

    // Round 2: Fully connect graph. But remember the digest of the leader. Note that this
    // round is the only one with 4 certificates.
    let (leader_2_digest, certificate) =
        mock_certificate(&committee, ids.first().unwrap().clone(), 2, parents.clone());
    certificates.push_back(certificate);

    let nodes: Vec<_> = ids.iter().skip(1).cloned().collect();
    let (out, mut parents) = make_optimal_certificates(&committee, 2..=2, &parents, &nodes);
    certificates.extend(out);

    // Round 3: Only node 0 links to the leader of round 2.
    let mut next_parents = BTreeSet::new();

    let name = ids.get(1).unwrap().clone();
    let (digest, certificate) = mock_certificate(&committee, name, 3, parents.clone());
    certificates.push_back(certificate);
    next_parents.insert(digest);

    let name = ids.get(2).unwrap().clone();
    let (digest, certificate) = mock_certificate(&committee, name, 3, parents.clone());
    certificates.push_back(certificate);
    next_parents.insert(digest);

    let name = ids.first().unwrap().clone();
    parents.insert(leader_2_digest);
    let (digest, certificate) = mock_certificate(&committee, name, 3, parents.clone());
    certificates.push_back(certificate);
    next_parents.insert(digest);

    parents.clone_from(&next_parents);

    // Rounds 4: Fully connected graph. This is the where we "boost" the leader.
    let nodes: Vec<_> = ids.to_vec();
    let (out, parents) = make_optimal_certificates(&committee, 4..=4, &parents, &nodes);
    certificates.extend(out);

    // Round 5: Send f+1 certificates to trigger the commit of leader 4.
    let (_, certificate) =
        mock_certificate(&committee, ids.first().unwrap().clone(), 5, parents.clone());
    certificates.push_back(certificate);
    let (_, certificate) = mock_certificate(&committee, ids.get(1).unwrap().clone(), 5, parents);
    certificates.push_back(certificate);

    let config = fixture.authorities().next().unwrap().consensus_config();
    let store = config.node_storage().clone();
    let metrics = Arc::new(ConsensusMetrics::default());

    let bullshark = Bullshark::new(
        committee.clone(),
        store.clone(),
        metrics.clone(),
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee.clone(), LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    let cb = ConsensusBus::new();
    let dummy_parent = SealedHeader::new(ExecHeader::default(), B256::default());
    cb.recent_blocks().send_modify(|blocks| blocks.push_latest(dummy_parent));
    let mut rx_output = cb.sequence().subscribe();
    Consensus::spawn(config, &cb, bullshark, &TaskManager::default());
    let cb_clone = cb.clone();
    tokio::spawn(async move {
        let mut rx_primary = cb_clone.committed_certificates().subscribe();
        while rx_primary.recv().await.is_some() {}
    });

    // Feed all certificates to the consensus. Only the last certificate should trigger
    // commits, so the task should not block.
    while let Some(certificate) = certificates.pop_front() {
        cb.new_certificates().send(certificate).await.unwrap();
    }

    // We should commit 2 leaders (rounds 2 and 4).
    let committed_sub_dag: CommittedSubDag = rx_output.recv().await.unwrap();
    let mut sequence = committed_sub_dag.certificates.into_iter();
    for _ in 1..=3 {
        let output = sequence.next().unwrap();
        assert_eq!(output.round(), 1);
    }
    let output = sequence.next().unwrap();
    assert_eq!(output.round(), 2);

    // AND all scores are zero for leader 2 , as this is the first commit
    assert_eq!(committed_sub_dag.reputation_score.total_authorities(), 4);
    assert!(committed_sub_dag.reputation_score.all_zero());

    let committed_sub_dag: CommittedSubDag = rx_output.recv().await.unwrap();
    let mut sequence = committed_sub_dag.certificates.into_iter();
    for _ in 1..=3 {
        let output = sequence.next().unwrap();
        assert_eq!(output.round(), 2);
    }
    for _ in 1..=3 {
        let output = sequence.next().unwrap();
        assert_eq!(output.round(), 3);
    }
    let output = sequence.next().unwrap();
    assert_eq!(output.round(), 4);

    // AND scores should be updated with everyone that has voted for leader of round 2.
    // Only node 0 has voted for the leader of this round, so only their score should exist
    // with value 1, and everything else should be zero.
    assert_eq!(committed_sub_dag.reputation_score.total_authorities(), 4);

    let node_0_name: AuthorityIdentifier = ids.first().unwrap().clone();
    committed_sub_dag.reputation_score.scores_per_authority.iter().for_each(|(key, score)| {
        if *key == node_0_name {
            assert_eq!(*score, 1_u64);
        } else {
            assert_eq!(*score, 0_u64);
        }
    });
}

/// Run for 7 dag rounds. Node 0 (the leader of round 2) is missing for rounds 1 and 2,
/// and reappears from round 3.
#[tokio::test]
async fn missing_leader() {
    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();
    let mut ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    ids.sort();

    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();

    let mut certificates = VecDeque::new();

    // Remove the leader for rounds 1 and 2.
    let nodes: Vec<_> = ids.iter().skip(1).cloned().collect();
    let (out, parents) = make_optimal_certificates(&committee, 1..=2, &genesis, &nodes);
    certificates.extend(out);

    // Add back the leader for rounds 3 and 4.
    let (out, parents) = make_optimal_certificates(&committee, 3..=4, &parents, &ids);
    certificates.extend(out);

    // Add f+1 certificates of round 5 to commit the leader of round 4.
    let (_, certificate) =
        mock_certificate(&committee, ids.first().unwrap().clone(), 5, parents.clone());
    certificates.push_back(certificate);
    let (_, certificate) = mock_certificate(&committee, ids.get(1).unwrap().clone(), 5, parents);
    certificates.push_back(certificate);

    let config = fixture.authorities().next().unwrap().consensus_config();
    let store = config.node_storage().clone();
    let metrics = Arc::new(ConsensusMetrics::default());
    let bullshark = Bullshark::new(
        committee.clone(),
        store.clone(),
        metrics.clone(),
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee.clone(), LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    let cb = ConsensusBus::new();
    let dummy_parent = SealedHeader::new(ExecHeader::default(), B256::default());
    cb.recent_blocks().send_modify(|blocks| blocks.push_latest(dummy_parent));
    let mut rx_output = cb.sequence().subscribe();
    Consensus::spawn(config, &cb, bullshark, &TaskManager::default());
    let cb_clone = cb.clone();
    tokio::spawn(async move {
        let mut rx_primary = cb_clone.committed_certificates().subscribe();
        while rx_primary.recv().await.is_some() {}
    });

    // Feed all certificates to the consensus. We should only commit upon receiving the last
    // certificate, so calls below should not block the task.
    while let Some(certificate) = certificates.pop_front() {
        cb.new_certificates().send(certificate).await.unwrap();
    }

    // Ensure the commit sequence is as expected.
    let committed_sub_dag: CommittedSubDag = rx_output.recv().await.unwrap();
    let mut sequence = committed_sub_dag.certificates.into_iter();
    for _ in 1..=3 {
        let output = sequence.next().unwrap();
        assert_eq!(output.round(), 1);
    }
    for _ in 1..=3 {
        let output = sequence.next().unwrap();
        assert_eq!(output.round(), 2);
    }
    for _ in 1..=4 {
        let output = sequence.next().unwrap();
        assert_eq!(output.round(), 3);
    }
    let output = sequence.next().unwrap();
    assert_eq!(output.round(), 4);

    // AND all scores are zero since this is the first commit that has happened
    assert!(committed_sub_dag.reputation_score.all_zero());
}

/// Run for 11 dag rounds in ideal conditions (all nodes reference all other nodes).
/// Every two rounds (on odd rounds), restart consensus and check consistency.
#[tokio::test]
async fn committed_round_after_restart() {
    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();
    let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    let epoch = committee.epoch();

    // Make certificates for rounds 1 to 11.
    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();
    let (certificates, _) = make_certificates_with_epoch(&committee, 1..=11, epoch, &genesis, &ids);

    let config = fixture.authorities().next().unwrap().consensus_config();
    let store = config.node_storage().clone();

    for input_round in (1..=11usize).step_by(2) {
        let metrics = Arc::new(ConsensusMetrics::default());

        let bullshark = Bullshark::new(
            committee.clone(),
            store.clone(),
            metrics.clone(),
            NUM_SUB_DAGS_PER_SCHEDULE,
            LeaderSchedule::new(committee.clone(), LeaderSwapTable::default()),
            DEFAULT_BAD_NODES_STAKE_THRESHOLD,
        );

        let cb = ConsensusBus::new();
        let dummy_parent = SealedHeader::new(ExecHeader::default(), B256::default());
        cb.recent_blocks().send_modify(|blocks| blocks.push_latest(dummy_parent));
        let mut rx_primary = cb.committed_certificates().subscribe();
        let mut rx_output = cb.sequence().subscribe();
        let mut task_manager = TaskManager::default();
        Consensus::spawn(config.clone(), &cb, bullshark, &task_manager);

        // When `input_round` is 2 * r + 1, r > 1, the previous commit round would be 2 * (r - 1),
        // and the expected commit round after sending in certificates up to `input_round` would
        // be 2 * r.

        let last_committed_round = *cb.committed_round_updates().borrow() as usize;
        assert_eq!(last_committed_round, input_round.saturating_sub(3));
        info!("Consensus started at last_committed_round={last_committed_round}");

        // Feed certificates from two rounds into consensus.
        let start_index = input_round.saturating_sub(2) * committee.size();
        let end_index = input_round * committee.size();
        for cert in certificates.iter().take(end_index).skip(start_index) {
            store.write(cert.clone()).unwrap();
            cb.new_certificates().send(cert.clone()).await.unwrap();
        }
        info!("Sent certificates {start_index} ~ {end_index} to consensus");

        // There should only be one new item in the output streams.
        if input_round > 1 {
            let committed = rx_output.recv().await.unwrap();
            info!("Received output from consensus, committed_round={}", committed.leader.round());
            store.write_subdag_for_test(input_round as u64, committed);
            let (round, _certs) = rx_primary.recv().await.unwrap();
            info!("Received committed certificates from consensus, committed_round={round}",);
        }

        // After sending inputs up to round 2 * r + 1 to consensus, round 2 * r should have been
        // committed.
        assert_eq!(*cb.committed_round_updates().borrow() as usize, input_round.saturating_sub(1),);
        info!("Committed round adanced to {}", input_round.saturating_sub(1));

        // Shutdown consensus and wait for it to stop.
        fixture.notify_shutdown();
        let _ = task_manager.join(Notifier::default()).await;
    }
}

/// Advance the DAG for 4 rounds, commit, and then send a certificate
/// from round 2. Certificate 2 should not get committed.
#[tokio::test]
async fn delayed_certificates_are_rejected() {
    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();
    let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    let epoch = committee.epoch();
    let gc_depth = 10;

    // Make certificates for rounds 1 to 11.
    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();
    let metrics = Arc::new(ConsensusMetrics::default());
    let (certificates, _) = make_certificates_with_epoch(&committee, 1..=5, epoch, &genesis, &ids);

    let db = temp_dir();
    let store = open_db(db.path());
    let mut state = ConsensusState::new(metrics.clone(), gc_depth);

    let mut bullshark = Bullshark::new(
        committee.clone(),
        store,
        metrics,
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee, LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    // Populate DAG with the rounds up to round 5 so we trigger commits
    let mut all_subdags = Vec::new();
    for certificate in certificates.clone() {
        let (_, committed_subdags) =
            bullshark.process_certificate(&mut state, certificate).unwrap();
        all_subdags.extend(committed_subdags);
    }

    // ensure the leaders of rounds 2 and 4 have been committed
    assert_eq!(all_subdags.drain(0..).len(), 2);

    // now populate again the certificates of round 2 and 3
    // Since we committed everything of rounds <= 4, then those certificates should get rejected.
    for certificate in certificates.iter().filter(|c| c.round() <= 3) {
        let (outcome, _) = bullshark.process_certificate(&mut state, certificate.clone()).unwrap();

        assert_eq!(outcome, Outcome::CertificateBelowCommitRound);
    }
}

#[tokio::test]
async fn submitting_equivocating_certificate_should_error() {
    const NUM_SUB_DAGS_PER_SCHEDULE: u32 = 100;

    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();
    let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    let epoch = committee.epoch();
    let gc_depth = 10;

    // Make certificates for rounds 1 to 11.
    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();
    let metrics = Arc::new(ConsensusMetrics::default());
    let (certificates, _) = make_certificates_with_epoch(&committee, 1..=1, epoch, &genesis, &ids);

    let db = temp_dir();
    let store = open_db(db.path());
    let mut state = ConsensusState::new(metrics.clone(), gc_depth);
    let mut bullshark = Bullshark::new(
        committee.clone(),
        store,
        metrics,
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee.clone(), LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    // Populate DAG with all the certificates
    for certificate in certificates.clone() {
        let _ = bullshark.process_certificate(&mut state, certificate).unwrap();
    }

    // Try to re-submit the exact same certificates - no error should be produced.
    for certificate in certificates {
        let _ = bullshark.process_certificate(&mut state, certificate).unwrap();
    }

    // Try to submit certificates for same rounds but equivocating certificates (we just create
    // them with different epoch as a way to trigger the difference)
    let (certificates, _) = make_certificates_with_epoch(&committee, 1..=1, 100, &genesis, &ids);
    assert_eq!(certificates.len(), 4);

    for certificate in certificates {
        let err = bullshark.process_certificate(&mut state, certificate.clone()).unwrap_err();
        match err {
            ConsensusError::CertificateEquivocation(this_cert, _) => {
                assert_eq!(*this_cert, certificate);
            }
            err => panic!("Unexpected error returned: {err}"),
        }
    }
}

/// Advance the DAG for 50 rounds, while we change "schedule" for every 5 subdag commits.
#[tokio::test]
async fn reset_consensus_scores_on_every_schedule_change() {
    const NUM_SUB_DAGS_PER_SCHEDULE: u32 = 5;

    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee = fixture.committee();
    let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();
    let epoch = committee.epoch();
    let gc_depth = 10;

    // Make certificates for rounds 1 to 50.
    let genesis =
        Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();
    let metrics = Arc::new(ConsensusMetrics::default());
    let (certificates, _) = make_certificates_with_epoch(&committee, 1..=50, epoch, &genesis, &ids);

    let db = temp_dir();
    let store = open_db(db.path());
    let mut state = ConsensusState::new(metrics.clone(), gc_depth);
    let mut bullshark = Bullshark::new(
        committee.clone(),
        store,
        metrics,
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee, LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    // Populate DAG with the rounds up to round 50 so we trigger commits
    let mut all_subdags = Vec::new();
    for certificate in certificates {
        let (_, committed_subdags) =
            bullshark.process_certificate(&mut state, certificate).unwrap();
        all_subdags.extend(committed_subdags);
    }

    // ensure the leaders of rounds 2 and 4 have been committed
    let mut current_score = 0;
    for sub_dag in all_subdags {
        if (sub_dag.leader.round() / 2) % NUM_SUB_DAGS_PER_SCHEDULE == 0 {
            // On every 5th commit we reset the scores and count from the beginning with
            // scores updated to 1, as we expect now every node to have voted for the previous
            // leader.
            for score in sub_dag.reputation_score.scores_per_authority.values() {
                assert_eq!(*score as usize, 1);
            }
            current_score = 2;
        } else {
            for score in sub_dag.reputation_score.scores_per_authority.values() {
                assert_eq!(*score, current_score);
            }

            // On every other commit the scores get calculated incrementally with +1 score
            // for every commit.
            current_score += 1;

            if ((sub_dag.leader.round() / 2) + 1) % NUM_SUB_DAGS_PER_SCHEDULE == 0 {
                // if this is going to be the last score update for the current schedule, then
                // make sure that the `final_of_schedule` will be true
                assert!(sub_dag.reputation_score.final_of_schedule);
            } else {
                assert!(!sub_dag.reputation_score.final_of_schedule);
            }
        }
    }
}

/// Run for 4 dag rounds in ideal conditions (all nodes reference all other nodes). We should commit
/// the leader of round 2. Then shutdown consensus and restart in a new epoch.
#[tokio::test]
async fn restart_with_new_committee() {
    let mut fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let mut committee: Committee = fixture.committee();
    let ids: Vec<_> = fixture.authorities().map(|a| a.id()).collect();

    // Run for a few epochs.
    for epoch in 0..5 {
        let config = fixture.authorities().next().unwrap().consensus_config();
        let config = ConsensusConfig::new_with_committee_for_test(
            config.config().clone(),
            config.node_storage().clone(),
            config.key_config().clone(),
            committee.clone(),
            config.worker_cache().clone(),
            NetworkConfig::default(),
        )
        .unwrap();
        let store = config.node_storage().clone();
        store.clear().unwrap();
        let metrics = Arc::new(ConsensusMetrics::default());
        let bullshark = Bullshark::new(
            committee.clone(),
            store.clone(),
            metrics.clone(),
            NUM_SUB_DAGS_PER_SCHEDULE,
            LeaderSchedule::new(committee.clone(), LeaderSwapTable::default()),
            DEFAULT_BAD_NODES_STAKE_THRESHOLD,
        );

        let cb = ConsensusBus::new();
        let dummy_parent = SealedHeader::new(ExecHeader::default(), B256::default());
        cb.recent_blocks().send_modify(|blocks| blocks.push_latest(dummy_parent));
        let mut rx_output = cb.sequence().subscribe();
        let mut task_manager = TaskManager::default();
        Consensus::spawn(config.clone(), &cb, bullshark, &task_manager);
        let cb_clone = cb.clone();
        tokio::spawn(async move {
            let mut rx_primary = cb_clone.committed_certificates().subscribe();
            while rx_primary.recv().await.is_some() {}
        });

        // Make certificates for rounds 1 and 2.
        let genesis =
            Certificate::genesis(&committee).iter().map(|x| x.digest()).collect::<BTreeSet<_>>();
        let (mut certificates, next_parents) =
            make_certificates_with_epoch(&committee, 1..=2, epoch, &genesis, &ids);

        // Make two certificate (f+1) with round 3 to trigger the commits.
        let (_, certificate) = mock_certificate_with_epoch(
            &committee,
            ids.first().unwrap().clone(),
            3,
            epoch,
            next_parents.clone(),
        );
        certificates.push_back(certificate);
        let (_, certificate) = mock_certificate_with_epoch(
            &committee,
            ids.get(1).unwrap().clone(),
            3,
            epoch,
            next_parents,
        );
        certificates.push_back(certificate);

        // Feed all certificates to the consensus. Only the last certificate should trigger
        // commits, so the task should not block.
        while let Some(certificate) = certificates.pop_front() {
            cb.new_certificates().send(certificate).await.unwrap();
        }

        // Ensure the first 4 ordered certificates are from round 1 (they are the parents of the
        // committed leader); then the leader's certificate should be committed.
        let committed_sub_dag = rx_output.recv().await.unwrap();
        let mut sequence = committed_sub_dag.certificates.into_iter();
        for _ in 1..=4 {
            let output = sequence.next().unwrap();
            assert_eq!(output.epoch(), epoch);
            assert_eq!(output.round(), 1);
        }
        let output = sequence.next().unwrap();
        assert_eq!(output.epoch(), epoch);
        assert_eq!(output.round(), 2);

        // Move to the next epoch.
        committee = committee.advance_epoch_for_test(epoch + 1);
        fixture.update_committee(committee.clone());
        config.shutdown().notify();

        // Ensure consensus stopped.
        let _ = task_manager.join(Notifier::default()).await;
    }
}

/// The test ensures the following things:
/// * garbage collection is removing the certificates from lower rounds according to gc depth only
/// * no certificate will ever get committed past the gc round
/// * existing uncommitted certificates in DAG (ex from slow nodes where no-one references them) are
///   cleaned up.
#[tokio::test]
async fn garbage_collection_basic() {
    const GC_DEPTH: Round = 4;

    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee: Committee = fixture.committee();

    // We create certificates for rounds 1 to 7. For the authorities 1 to 3 the references
    // to previous rounds between them are fully connected - meaning that we always add all the
    // parents from previous round, except for the authority 4 which we consider it to be slow,
    // so no one refers to their certificates. Authority 4 still produces certificates, and uses
    // as parents all the certificates of the other authorities but never anyone else is
    // referring to its certificates. That will create a lone chain for authority 4. We should
    // not see any certificate committed for authority 4.
    let ids: Vec<AuthorityIdentifier> =
        committee.authorities().iter().map(|authority| authority.id()).collect();
    let slow_node = ids.get(3).unwrap().clone();
    let genesis = Certificate::genesis(&committee);

    let slow_nodes = vec![(slow_node.clone(), 0.0_f64)];
    let (certificates, _round_5_certificates) =
        make_certificates_with_slow_nodes(&committee, 1..=7, genesis, &ids, slow_nodes.as_slice());

    // Create Bullshark consensus engine
    let db = temp_dir();
    let store = open_db(db.path());

    let metrics = Arc::new(ConsensusMetrics::default());
    let mut state = ConsensusState::new(metrics.clone(), GC_DEPTH);
    let mut bullshark = Bullshark::new(
        committee.clone(),
        store,
        metrics,
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee, LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    // Now start feeding the certificates per round
    for c in certificates {
        let (_, sub_dags) = bullshark.process_certificate(&mut state, c).unwrap();

        sub_dags.iter().for_each(|sub_dag| {
            // ensure nothing has been committed for authority 4
            assert!(
                !sub_dag.certificates.iter().any(|c| c.header().author() == &slow_node),
                "Slow authority shouldn't be amongst the committed ones"
            );

            // Once leader of round 6 is committed then we know that garbage
            // collection has run. In this case no certificate of round 1 should exist.
            if sub_dag.leader.round() == 6 {
                assert_eq!(
                    state.dag.iter().filter(|(round, _)| **round <= 2).count(),
                    0,
                    "Didn't expect to still have certificates from round 1 and 2"
                );
            }

            // When we do commit for authorities, we always keep the certificates up to their latest
            // commit round + 1. Since we always commit for authorities 1 to 3 we expect to see no
            // certificates for them, but only for the slow authority 4 for which we never commit.
            // In this case the highest commit round for the authorities should be the
            // leader.round() - 1, except for the latest leader which should be
            // leader.round().
            for (_round, certificates) in
                state.dag.iter().filter(|(round, _)| **round <= sub_dag.leader.round())
            {
                assert_eq!(certificates.len(), 4, "We expect to have all the certificates");
            }
        })
    }
}

/// This test ensures that:
/// * a slow node will never commit anything until its certificates get linked by others
/// * certificates arriving bellow the gc round will never get committed
#[tokio::test]
async fn slow_node() {
    const GC_DEPTH: Round = 4;

    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee: Committee = fixture.committee();

    // We create certificates for rounds 1 to 8. For the authorities 1 to 3 the references
    // to previous rounds between them are fully connected - meaning that we always add all the
    // parents from previous round, except for the authority 4 which we consider it to be slow,
    // so no one refers to their certificates. Authority 4 still produces certificates, and uses
    // as parents all the certificates of the other authorities but never anyone else is
    // referring to its certificates. That will create a lone chain for authority 4. We should
    // not see any certificate committed for authority 4.
    let ids: Vec<AuthorityIdentifier> =
        committee.authorities().iter().map(|authority| authority.id()).collect();
    let slow_node = ids.get(3).unwrap().clone();
    let genesis = Certificate::genesis(&committee);

    let slow_nodes = vec![(slow_node.clone(), 0.0_f64)];
    let (certificates, round_8_certificates) =
        make_certificates_with_slow_nodes(&committee, 1..=8, genesis, &ids, slow_nodes.as_slice());

    let mut certificates: VecDeque<Certificate> = certificates;
    let mut slow_node_certificates = VecDeque::new();

    // Now we keep only the certificates from authorities 1-3
    certificates.retain(|c| {
        if c.origin() == &slow_node {
            // if it is slow node's add it to the dedicated vec
            slow_node_certificates.push_back(c.clone());
            return false;
        }
        true
    });

    // Create Bullshark consensus engine
    let db = temp_dir();
    let store = open_db(db.path());
    let metrics = Arc::new(ConsensusMetrics::default());
    let mut state = ConsensusState::new(metrics.clone(), GC_DEPTH);
    let mut bullshark = Bullshark::new(
        committee.clone(),
        store,
        metrics,
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee.clone(), LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    // Now start feeding the certificates per round up to 8. We expect to have
    // triggered a commit up to round 6 and gc round 1 & 2.
    for c in certificates {
        let _ = bullshark.process_certificate(&mut state, c.clone()).unwrap();
    }

    // We expect everything to have been cleaned up by standard gc until round 2 (included)
    assert_eq!(
        state.dag.iter().filter(|(round, _)| **round <= 2).count(),
        0,
        "Didn't expect to still have certificates from round 1 and 2"
    );

    // Now send the certificates of slow node. Now leader election can't happen for round 8
    // as we haven't sent yet certificates of round 9
    for c in slow_node_certificates {
        let _ = bullshark.process_certificate(&mut state, c).unwrap();
    }

    // Now create the certificates of round 9, and ensure everyone gives support to
    // leader of round 8 (the slow node - 4) so we can trigger a commit. Also since slow node
    // refers to always all the parents of previous rounds, there will be a link to the previous
    // leader, so commit should be triggered immediately.
    // It is reminded that the leader election for testing is round robin, thus we can
    // deterministically know the leader of each round.
    let (certificates, _) =
        make_certificates_with_slow_nodes(&committee, 9..=9, round_8_certificates, &ids, &[]);

    // send the certificates - they should trigger a commit.
    // We should see certificates for authorities 1-3 only for round 7
    // We should see certificates for authority 4 only for round > 1 , as round 1 should have been
    // garbage collected.
    let mut committed = false;
    for c in certificates {
        let (outcome, sub_dags) = bullshark.process_certificate(&mut state, c).unwrap();

        match outcome {
            Outcome::NotEnoughSupportForLeader => {}
            Outcome::LeaderBelowCommitRound => {}
            Outcome::Commit => {
                assert_eq!(sub_dags.len(), 1);

                let sub_dag = sub_dags.first().unwrap();

                for committed in &sub_dag.certificates {
                    assert!(
                        committed.round() >= 2,
                        "We don't expect to see any certificate below round 2 because of gc"
                    );
                }

                let slow_node_total =
                    sub_dag.certificates.iter().filter(|c| c.origin() == &slow_node).count();

                assert_eq!(slow_node_total, 4);

                committed = true;
            }
            _ => panic!("Unexpected outcome {outcome:?}"),
        }
    }

    assert!(committed, "We expect to have commit for round 8");
}

/// This test creates a DAG that:
/// * contains a leader that does not have enough support at round 2
/// * a leader is missing at round 4
/// * gc happens on commit of leader round 6
/// * a dead node (node 4) for the first 3 rounds
/// * a dead node (node 1) for the last 3 rounds
#[tokio::test]
async fn not_enough_support_and_missing_leaders_and_gc() {
    const GC_DEPTH: Round = 4;

    let fixture = CommitteeFixture::builder(MemDatabase::default).build();
    let committee: Committee = fixture.committee();

    let ids: Vec<AuthorityIdentifier> =
        committee.authorities().iter().map(|authority| authority.id()).collect();

    // take the first 3 nodes only - 4th one won't propose anything
    let keys_with_dead_node = ids[0..=2].to_vec();
    let slow_node = ids.get(3).unwrap().clone();
    let slow_nodes = vec![(slow_node, 0.0_f64)];
    let genesis = Certificate::genesis(&committee);

    let (mut certificates, round_2_certificates) = make_certificates_with_slow_nodes(
        &committee,
        1..=2,
        genesis,
        &keys_with_dead_node,
        &slow_nodes,
    );

    // on round 3 we'll create certificates that don't provide f+1 support to round 2.
    let mut round_3_certificates: Vec<Certificate> = Vec::new();
    let first_node = keys_with_dead_node.first().unwrap();
    for id in &keys_with_dead_node {
        // Only the first one will provide support to it's own certificate apart from the others
        if id == first_node {
            let parents =
                round_2_certificates.iter().map(|cert| cert.digest()).collect::<BTreeSet<_>>();
            let (_, certificate) = mock_certificate(&committee, id.clone(), 3, parents);
            round_3_certificates.push(certificate);
        } else {
            // we filter out the round 2 leader
            let parents = round_2_certificates
                .iter()
                .filter(|cert| cert.origin() != first_node)
                .map(|cert| cert.digest())
                .collect::<BTreeSet<_>>();
            let (_, certificate) = mock_certificate(&committee, id.clone(), 3, parents);
            round_3_certificates.push(certificate);
        }
    }

    // on round 4 we create a missing leader (node 2)
    let mut round_4_certificates = Vec::new();
    let missing_leader = &ids[1];
    for id in ids.iter().filter(|a| *a != missing_leader) {
        let parents =
            round_3_certificates.iter().map(|cert| cert.digest()).collect::<BTreeSet<_>>();
        let (_, certificate) = mock_certificate(&committee, id.clone(), 4, parents);
        round_4_certificates.push(certificate);
    }

    // now from round 5 to 7 create all certificates. Node 1 is now a slow node and won't create
    // referrencies to the certificates of that one.
    let slow_node = ids.first().unwrap().clone();
    let slow_nodes = vec![(slow_node, 0.0_f64)];

    let (certificates_5_to_7, _round_7_certificates) = make_certificates_with_slow_nodes(
        &committee,
        5..=7,
        round_4_certificates.clone(),
        &ids,
        &slow_nodes,
    );

    // now send all certificates to Bullshark
    certificates.extend(round_3_certificates);
    certificates.extend(round_4_certificates);
    certificates.extend(certificates_5_to_7);

    // Create Bullshark consensus engine
    let db = temp_dir();
    let store = open_db(db.path());
    let metrics = Arc::new(ConsensusMetrics::default());
    let mut state = ConsensusState::new(metrics.clone(), GC_DEPTH);
    let mut bullshark = Bullshark::new(
        committee.clone(),
        store,
        metrics,
        NUM_SUB_DAGS_PER_SCHEDULE,
        LeaderSchedule::new(committee, LeaderSwapTable::default()),
        DEFAULT_BAD_NODES_STAKE_THRESHOLD,
    );

    let mut committed = false;
    for c in &certificates {
        let (outcome, sub_dags) = bullshark.process_certificate(&mut state, c.clone()).unwrap();

        // We expect leader of round 2 to not have enough support from certificates of round 3,
        // thus no commit should happen and every attempt should return a not enough support outcome
        if c.round() == 3 {
            assert_eq!(outcome, Outcome::NotEnoughSupportForLeader);
        }

        // We don't expect to have a leader at round 4, thus any addition of certificate on round 5
        // should not find a leader.
        if c.round() == 5 {
            assert_eq!(outcome, Outcome::LeaderNotFound);
        }

        // Leader election is triggered when populating certificates of odd rounds.
        if c.round() == 1 || c.round() == 2 || c.round() == 4 {
            assert_eq!(outcome, Outcome::NoLeaderElectedForOddRound);
        }

        // We do not expect any commit when inserting certificates below or equal round 6
        if c.round() <= 6 {
            assert_eq!(sub_dags.len(), 0);
        }

        // At round 7 we expect to trigger a commit
        if c.round() == 7 {
            match outcome {
                Outcome::NotEnoughSupportForLeader => {}
                Outcome::LeaderBelowCommitRound => {}
                Outcome::Commit => {
                    // we expect now to commit two sub dags, those with leader 6 and 2.
                    assert_eq!(sub_dags.len(), 2);

                    assert_eq!(sub_dags[0].leader.round(), 2);
                    assert_eq!(sub_dags[1].leader.round(), 6);

                    assert_eq!(sub_dags[0].certificates.len(), 4);
                    assert_eq!(sub_dags[1].certificates.len(), 10);

                    // And GC has collected everything up to round 5.
                    assert_eq!(state.dag.len(), 5);

                    for (round, entries) in state.dag.iter() {
                        assert!(*round >= 3, "{}", format!("Round detected: {round}"));

                        if *round == 3 || *round == 4 {
                            assert_eq!(entries.len(), 3);
                        } else if *round == 5 || *round == 6 {
                            assert_eq!(entries.len(), 4);
                        } else {
                            assert_eq!(entries.len(), 2);
                        }
                    }

                    committed = true;
                }
                _ => panic!("Unexpected outcome: {outcome:?}"),
            }
        }
    }

    assert!(committed);
}
