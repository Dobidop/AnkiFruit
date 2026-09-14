// Typed wrappers for the handful of backend methods the shell needs so far.
// Method names match the routing table generated in rust/ankifruit-core; a
// typo surfaces as a 404 from the server rather than a silent no-op.

import { create } from "@bufbuild/protobuf";
import { EmptySchema } from "../gen/anki/generic_pb.js";
import {
  CloseCollectionRequestSchema,
  OpenCollectionRequestSchema,
} from "../gen/anki/collection_pb.js";
import { DeckNamesSchema, GetDeckNamesRequestSchema } from "../gen/anki/decks_pb.js";
import { call } from "./client.js";

export interface CollectionPaths {
  collectionPath: string;
  mediaFolderPath: string;
  mediaDbPath: string;
}

/** Opens a collection, creating it if the file does not yet exist. */
export async function openCollection(paths: CollectionPaths): Promise<void> {
  await call(
    "openCollection",
    OpenCollectionRequestSchema,
    create(OpenCollectionRequestSchema, paths),
    EmptySchema,
  );
}

export async function closeCollection(downgradeToSchema11 = false): Promise<void> {
  await call(
    "closeCollection",
    CloseCollectionRequestSchema,
    create(CloseCollectionRequestSchema, { downgradeToSchema11 }),
    EmptySchema,
  );
}

export interface Deck {
  id: bigint;
  name: string;
}

export async function getDeckNames(options?: {
  skipEmptyDefault?: boolean;
  includeFiltered?: boolean;
}): Promise<Deck[]> {
  const response = await call(
    "getDeckNames",
    GetDeckNamesRequestSchema,
    create(GetDeckNamesRequestSchema, {
      skipEmptyDefault: options?.skipEmptyDefault ?? false,
      includeFiltered: options?.includeFiltered ?? true,
    }),
    DeckNamesSchema,
  );
  return response.entries.map((entry) => ({ id: entry.id, name: entry.name }));
}
