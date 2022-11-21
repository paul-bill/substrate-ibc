use crate::{
	context::Context,
	prelude::{format, String, ToString},
	Acknowledgements, ChannelCounter, Channels, ChannelsConnection, ClientProcessedHeights,
	ClientProcessedTimes, Config, IbcChannelId, NextSequenceAck, NextSequenceRecv,
	NextSequenceSend, PacketCommitment, PacketReceipt, Pallet, Store, REVISION_NUMBER,
};
use sp_std::{boxed::Box, vec, vec::Vec};

use core::{str::FromStr, time::Duration};
use ibc::{
	core::{
		ics02_client::{
			client_state::ClientState, consensus_state::ConsensusState, context::ClientReader,
		},
		ics03_connection::{
			connection::ConnectionEnd, context::ConnectionReader, error::Error as Ics03Error,
		},
		ics04_channel::{
			channel::ChannelEnd,
			commitment::{
				AcknowledgementCommitment as IbcAcknowledgementCommitment,
				PacketCommitment as IbcPacketCommitment,
			},
			context::{ChannelKeeper, ChannelReader},
			error::Error as Ics04Error,
			packet::{Receipt, Sequence},
		},
		ics24_host::{
			identifier::{ChannelId, ClientId, ConnectionId, PortId},
			path::{
				AcksPath, ChannelEndsPath, CommitmentsPath, ConnectionsPath, ReceiptsPath,
				SeqAcksPath, SeqRecvsPath, SeqSendsPath,
			},
			Path,
		},
	},
	timestamp::Timestamp,
	Height,
};
use ibc_proto::protobuf::Protobuf;

impl<T: Config> ChannelReader for Context<T> {
	fn channel_end(&self, port_id: &PortId, channel_id: &ChannelId) -> Result<ChannelEnd, Ics04Error> {
		let channel_end_path = ChannelEndsPath(port_id.clone(), channel_id.clone())
			.to_string()
			.as_bytes()
			.to_vec();

		if <Channels<T>>::contains_key(&channel_end_path) {
			let data = <Channels<T>>::get(&channel_end_path);

			let channel_end = ChannelEnd::decode_vec(&data)
				.map_err(|_| Ics04Error::channel_not_found(port_id.clone(), channel_id.clone()))?;

			Ok(channel_end)
		} else {
			Err(Ics04Error::channel_not_found(port_id.clone(), channel_id.clone()))
		}
	}

	fn connection_end(&self, connection_id: &ConnectionId) -> Result<ConnectionEnd, Ics04Error> {
		let context = Context::<T>::new();
		ConnectionReader::connection_end(&context, connection_id)
			.map_err(Ics04Error::ics03_connection)
	}

	/// Returns the `ChannelsConnection` for the given identifier `conn_id`.
	fn connection_channels(&self, conn_id: &ConnectionId) -> Result<Vec<(PortId, ChannelId)>, Ics04Error> {
		let connections_path = ConnectionsPath(conn_id.clone()).to_string().as_bytes().to_vec();

		if <ChannelsConnection<T>>::contains_key(&connections_path) {
			let channel_ends_paths = <ChannelsConnection<T>>::get(&connections_path);

			let mut result = vec![];

			for item in channel_ends_paths.into_iter() {
				let raw_path = String::from_utf8(item).map_err(|e| {
					Ics04Error::other(format!(
						"Decode ChannelEnds Paths String format failed: {:?}",
						e
					))
				})?;
				let path = Path::from_str(&raw_path).map_err(|e| {
					Ics04Error::other(format!("Decode ChannelEnds Path format Failed: {:?}", e))
				})?;
				match path {
					Path::ChannelEnds(channel_ends_path) => {
						let ChannelEndsPath(port_id, channel_id) = channel_ends_path;
						result.push((port_id, channel_id));
					},
					_ => unimplemented!(),
				}
			}
			Ok(result)
		} else {
			Err(Ics04Error::connection_not_open(conn_id.clone()))
		}
	}

	fn client_state(&self, client_id: &ClientId) -> Result<Box<dyn ClientState>, Ics04Error> {
		let context = Context::<T>::new();
		ClientReader::client_state(&context, client_id)
			.map_err(|e| Ics04Error::ics03_connection(Ics03Error::ics02_client(e)))
	}

	fn client_consensus_state(
		&self,
		client_id: &ClientId,
		height: Height,
	) -> Result<Box<dyn ConsensusState>, Ics04Error> {
		let context = Context::<T>::new();
		ClientReader::consensus_state(&context, client_id, height)
			.map_err(|e| Ics04Error::ics03_connection(Ics03Error::ics02_client(e)))
	}

	fn get_next_sequence_send(
		&self,
		port_id: &PortId,
		channel_id: &ChannelId,
	) -> Result<Sequence, Ics04Error> {
		let seq_sends_path = SeqSendsPath(port_id.clone(), channel_id.clone())
			.to_string()
			.as_bytes()
			.to_vec();

		if <NextSequenceSend<T>>::contains_key(&seq_sends_path) {
			let sequence = <NextSequenceSend<T>>::get(&seq_sends_path);
			Ok(Sequence::from(sequence))
		} else {
			Err(Ics04Error::missing_next_send_seq(port_id.clone(), channel_id.clone()))
		}
	}

	fn get_next_sequence_recv(
		&self,
		port_id: &PortId,
		channel_id: &ChannelId,
	) -> Result<Sequence, Ics04Error> {
		let seq_recvs_path = SeqRecvsPath(port_id.clone(), channel_id.clone())
			.to_string()
			.as_bytes()
			.to_vec();

		if <NextSequenceRecv<T>>::contains_key(&seq_recvs_path) {
			let sequence = <NextSequenceRecv<T>>::get(&seq_recvs_path);
			Ok(Sequence::from(sequence))
		} else {
			Err(Ics04Error::missing_next_recv_seq(port_id.clone(), channel_id.clone()))
		}
	}

	fn get_next_sequence_ack(
		&self,
		port_id: &PortId,
		channel_id: &ChannelId,
	) -> Result<Sequence, Ics04Error> {
		let seq_acks_path =
			SeqAcksPath(port_id.clone(), channel_id.clone()).to_string().as_bytes().to_vec();

		if <NextSequenceAck<T>>::contains_key(&seq_acks_path) {
			let sequence = <NextSequenceAck<T>>::get(&seq_acks_path);

			Ok(Sequence::from(sequence))
		} else {
			Err(Ics04Error::missing_next_ack_seq(port_id.clone(), channel_id.clone()))
		}
	}

	/// Returns the `PacketCommitment` for the given identifier `(PortId, ChannelId, Sequence)`.
	fn get_packet_commitment(
		&self,
		port_id: &PortId,
		channel_id: &ChannelId,
		seq: Sequence,
	) -> Result<IbcPacketCommitment, Ics04Error> {
		let packet_commitments_path = CommitmentsPath {
			port_id: port_id.clone(),
			channel_id: channel_id.clone(),
			sequence: seq,
		}
		.to_string()
		.as_bytes()
		.to_vec();

		if <PacketCommitment<T>>::contains_key(&packet_commitments_path) {
			let data = <PacketCommitment<T>>::get(&packet_commitments_path);
			let packet_commitment = IbcPacketCommitment::from(data);
			Ok(packet_commitment)
		} else {
			Err(Ics04Error::packet_commitment_not_found(seq))
		}
	}

	fn get_packet_receipt(
		&self,
		port_id: &PortId,
		channel_id: &ChannelId,
		seq: Sequence,
	) -> Result<Receipt, Ics04Error> {
		let packet_receipt_path = ReceiptsPath {
			port_id: port_id.clone(),
			channel_id: channel_id.clone(),
			sequence: seq,
		}
		.to_string()
		.as_bytes()
		.to_vec();

		if <PacketReceipt<T>>::contains_key(&packet_receipt_path) {
			let data = <PacketReceipt<T>>::get(&packet_receipt_path);
			let data = String::from_utf8(data)
				.map_err(|e| Ics04Error::other(format!("Decode packet receipt failed: {:?}", e)))?;
			let data = match data.as_ref() {
				"Ok" => Receipt::Ok,
				e => return Err(Ics04Error::other(format!("Unknown Receipts {:?}", e))),
			};
			Ok(data)
		} else {
			Err(Ics04Error::packet_receipt_not_found(seq))
		}
	}

	/// Returns the `Acknowledgements` for the given identifier `(PortId, ChannelId, Sequence)`.
	fn get_packet_acknowledgement(
		&self,
		port_id: &PortId,
		channel_id: &ChannelId,
		seq: Sequence,
	) -> Result<IbcAcknowledgementCommitment, Ics04Error> {
		let acks_path =
			AcksPath { port_id: port_id.clone(), channel_id: channel_id.clone(), sequence: seq }
				.to_string()
				.as_bytes()
				.to_vec();

		if <Acknowledgements<T>>::contains_key(&acks_path) {
			let data = <Acknowledgements<T>>::get(&acks_path);
			let acknowledgement = IbcAcknowledgementCommitment::from(data);

			Ok(acknowledgement)
		} else {
			Err(Ics04Error::packet_acknowledgement_not_found(seq))
		}
	}

	/// A hashing function for packet commitments
	fn hash(&self, value: Vec<u8>) -> Vec<u8> {
		sp_io::hashing::sha2_256(&value).to_vec()
	}

	/// Returns the current height of the local chain.
	fn host_height(&self) -> Height {
		let block_number = format!("{:?}", <frame_system::Pallet<T>>::block_number());
		let current_height: u64 = block_number.parse().unwrap_or_default();
		Height::new(REVISION_NUMBER, current_height).unwrap()
	}

	/// Returns the `AnyConsensusState` for the given identifier `height`.
	fn host_consensus_state(&self, height: Height) -> Result<Box<dyn ConsensusState>, Ics04Error> {
		let context = Context::<T>::new();
		ConnectionReader::host_consensus_state(&context, height)
			.map_err(Ics04Error::ics03_connection)
	}

	fn pending_host_consensus_state(&self) -> Result<Box<dyn ConsensusState>, Ics04Error> {
		let context = Context::<T>::new();
		ClientReader::pending_host_consensus_state(&context)
			.map_err(|e| Ics04Error::ics03_connection(Ics03Error::ics02_client(e)))
	}

	/// Returns the `ClientProcessedTimes` for the given identifier `client_id` & `height`.
	fn client_update_time(&self, client_id: &ClientId, height: Height) -> Result<Timestamp, Ics04Error> {
		if <ClientProcessedTimes<T>>::contains_key(
			client_id.as_bytes(),
			height
				.encode_vec()
				.map_err(|e| Ics04Error::other(format!("Encode height failed: {:?}", e)))?,
		) {
			let time = <ClientProcessedTimes<T>>::get(
				client_id.as_bytes(),
				height
					.encode_vec()
					.map_err(|e| Ics04Error::other(format!("Encode height failed: {:?}", e)))?,
			);
			let timestamp = String::from_utf8(time).map_err(|e| {
				Ics04Error::other(format!("Decode timestamp format String failed: {:?}", e))
			})?;
			let time: Timestamp = serde_json::from_str(&timestamp)
				.map_err(|e| Ics04Error::other(format!("Decode timestamp  failed: {:?}", e)))?;
			Ok(time)
		} else {
			Err(Ics04Error::processed_time_not_found(client_id.clone(), height))
		}
	}

	fn client_update_height(&self, client_id: &ClientId, height: Height) -> Result<Height, Ics04Error> {
		if <ClientProcessedHeights<T>>::contains_key(
			client_id.as_bytes(),
			height
				.encode_vec()
				.map_err(|e| Ics04Error::other(format!("Encode height failed: {:?}", e)))?,
		) {
			let host_height = <ClientProcessedHeights<T>>::get(
				client_id.as_bytes(),
				height
					.encode_vec()
					.map_err(|e| Ics04Error::other(format!("Encode height failed: {:?}", e)))?,
			);
			let host_height = Height::decode(&mut &host_height[..])
				.map_err(|e| Ics04Error::other(format!("Decode Host height failed: {:?}", e)))?;
			Ok(host_height)
		} else {
			Err(Ics04Error::processed_height_not_found(client_id.clone(), height))
		}
	}

	/// Returns a counter on the number of channel ids have been created thus far.
	/// The value of this counter should increase only via method
	/// `ChannelKeeper::increase_channel_counter`.
	fn channel_counter(&self) -> Result<u64, Ics04Error> {
		Ok(<Pallet<T> as Store>::ChannelCounter::get())
	}

	fn max_expected_time_per_block(&self) -> Duration {
		Duration::from_secs(6)
	}
}

impl<T: Config> ChannelKeeper for Context<T> {
	fn store_packet_commitment(
		&mut self,
		port_id: PortId,
		channel_id: ChannelId,
		seq: Sequence,
		commitment: IbcPacketCommitment,
	) -> Result<(), Ics04Error> {
		let packet_commitments_path = CommitmentsPath {
			port_id: port_id.clone(),
			channel_id: channel_id.clone(),
			sequence: seq,
		}
		.to_string()
		.as_bytes()
		.to_vec();

		<PacketCommitment<T>>::insert(packet_commitments_path, commitment.into_vec());

		Ok(())
	}

	fn delete_packet_commitment(
		&mut self,
		port_id: &PortId,
		channel_id: &ChannelId,
		seq: Sequence,
	) -> Result<(), Ics04Error> {
		let packet_commitments_path = CommitmentsPath {
			port_id: port_id.clone(),
			channel_id: channel_id.clone(),
			sequence: seq,
		}
		.to_string()
		.as_bytes()
		.to_vec();

		<PacketCommitment<T>>::remove(&packet_commitments_path);

		Ok(())
	}

	fn store_packet_receipt(
		&mut self,
		port_id: PortId,
		channel_id: ChannelId,
		seq: Sequence,
		receipt: Receipt,
	) -> Result<(), Ics04Error> {
		let packet_receipt_path = ReceiptsPath {
			port_id: port_id.clone(),
			channel_id: channel_id.clone(),
			sequence: seq,
		}
		.to_string()
		.as_bytes()
		.to_vec();

		let receipt = match receipt {
			Receipt::Ok => "Ok".as_bytes().to_vec(),
		};

		<PacketReceipt<T>>::insert(packet_receipt_path, receipt);

		Ok(())
	}

	fn store_packet_acknowledgement(
		&mut self,
		port_id: PortId,
		channel_id: ChannelId,
		seq: Sequence,
		ack_commitment: IbcAcknowledgementCommitment,
	) -> Result<(), Ics04Error> {
		let acks_path =
			AcksPath { port_id: port_id.clone(), channel_id: channel_id.clone(), sequence: seq }
				.to_string()
				.as_bytes()
				.to_vec();

		<Acknowledgements<T>>::insert(&acks_path, ack_commitment.into_vec());

		Ok(())
	}

	fn delete_packet_acknowledgement(
		&mut self,
		port_id: &PortId,
		channel_id: &ChannelId,
		seq: Sequence,
	) -> Result<(), Ics04Error> {
		let acks_path =
			AcksPath { port_id: port_id.clone(), channel_id: channel_id.clone(), sequence: seq }
				.to_string()
				.as_bytes()
				.to_vec();

		<Acknowledgements<T>>::remove(&acks_path);

		Ok(())
	}

	fn store_connection_channels(
		&mut self,
		conn_id: ConnectionId,
		port_id: PortId,
		channel_id: ChannelId,
	) -> Result<(), Ics04Error> {
		let connections_path = ConnectionsPath(conn_id).to_string().as_bytes().to_vec();
		let channel_ends_path = ChannelEndsPath(port_id.clone(), channel_id.clone())
			.to_string()
			.as_bytes()
			.to_vec();

		if <ChannelsConnection<T>>::contains_key(&connections_path) {
			let _ = <ChannelsConnection<T>>::try_mutate(
				&connections_path,
				|val| -> Result<(), Ics04Error> {
					val.push(channel_ends_path.clone());
					Ok(())
				},
			)
			.map_err(|e| Ics04Error::other(format!("store connection channels failed: {:?}", e)));
		} else {
			<ChannelsConnection<T>>::insert(connections_path, vec![channel_ends_path]);
		}

		Ok(())
	}

	/// Stores the given channel_end at a path associated with the port_id and channel_id.
	fn store_channel(
		&mut self,
		port_id: PortId,
		channel_id: ChannelId,
		channel_end: ChannelEnd,
	) -> Result<(), Ics04Error> {
		let channel_end_path = ChannelEndsPath(port_id.clone(), channel_id.clone())
			.to_string()
			.as_bytes()
			.to_vec();
		let channel_end = channel_end
			.encode_vec()
			.map_err(|e| Ics04Error::other(format!("encode channel end failed: {:?}", e)))?;
		<Channels<T>>::insert(channel_end_path, channel_end);

		Ok(())
	}

	fn store_next_sequence_send(
		&mut self,
		port_id: PortId,
		channel_id: ChannelId,
		seq: Sequence,
	) -> Result<(), Ics04Error> {
		let seq_sends_path = SeqSendsPath(port_id.clone(), channel_id.clone())
			.to_string()
			.as_bytes()
			.to_vec();

		let sequence = u64::from(seq);

		<NextSequenceSend<T>>::insert(seq_sends_path, sequence);

		Ok(())
	}

	fn store_next_sequence_recv(
		&mut self,
		port_id: PortId,
		channel_id: ChannelId,
		seq: Sequence,
	) -> Result<(), Ics04Error> {
		let seq_recvs_path = SeqRecvsPath(port_id.clone(), channel_id.clone())
			.to_string()
			.as_bytes()
			.to_vec();
		let sequence = u64::from(seq);

		<NextSequenceRecv<T>>::insert(seq_recvs_path, sequence);

		Ok(())
	}

	fn store_next_sequence_ack(
		&mut self,
		port_id: PortId,
		channel_id: ChannelId,
		seq: Sequence,
	) -> Result<(), Ics04Error> {
		let seq_acks_path =
			SeqAcksPath(port_id.clone(), channel_id.clone()).to_string().as_bytes().to_vec();
		let sequence = u64::from(seq);

		<NextSequenceAck<T>>::insert(seq_acks_path, sequence);

		Ok(())
	}

	/// Called upon channel identifier creation (Init or Try message processing).
	/// Increases the counter which keeps track of how many channels have been created.
	/// Should never fail.
	fn increase_channel_counter(&mut self,) {
		let _ = <ChannelCounter<T>>::try_mutate(|val| -> Result<(), Ics04Error> {
			let new = val
				.checked_add(1)
				.ok_or(Ics04Error::other(format!("add channel counter overflow")))?;
			*val = new;
			Ok(())
		});
	}
}
