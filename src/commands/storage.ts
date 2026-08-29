import { invoke } from './ipc';
import type { FileItem } from '@types';

export const listFiles     = (relativePath = ''): Promise<FileItem[]> => invoke('list_files', { relativePath });
export const readTextFile  = (relativePath: string): Promise<string>   => invoke('read_text_file', { relativePath });
export const writeTextFile = (relativePath: string, content: string): Promise<boolean> =>
  invoke('write_text_file', { req: { relative_path: relativePath, content } });
