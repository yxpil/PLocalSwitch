import { invoke } from './ipc';
import type { FileItem } from '@types';

export const listFiles     = (relativePath = ''): Promise<FileItem[]> => invoke('list_files', { relative_path: relativePath });
export const readTextFile  = (relativePath: string): Promise<string>   => invoke('read_text_file', { relative_path: relativePath });
export const writeTextFile = (relativePath: string, content: string): Promise<boolean> =>
  invoke('write_text_file', { relative_path: relativePath, content });
