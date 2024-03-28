import * as path from "node:path";
import { BunFile } from "bun";

export async function readContractFileBytes(
  filePath: string,
): Promise<Uint8Array> {
  const file: BunFile = Bun.file(filePath);

  if (!await file.exists()) {
    throw new Error(`Contract file ${filePath} does not exist`);
  }

  const contents: ArrayBuffer = await file.arrayBuffer();

  return Promise.resolve(new Uint8Array(contents));
}

export const getFileNameWithoutExtension = (filePath: string) =>
  path.basename(filePath, path.extname(filePath));

export const snakeCaseToKebabCase = (str: string) => str.replace(/_/g, "-");

