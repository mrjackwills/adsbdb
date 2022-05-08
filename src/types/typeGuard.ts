import { ModeS, CallSign } from '../types';

export const isModeS = (input: unknown): input is ModeS => !!input && typeof input === 'string' && !!input.match(/^[a-f0-9]{6}$/i);

export const isCallSign = (input: unknown): input is CallSign => !!input && typeof input === 'string' && !!input.match(/^[A-Z0-9]{4,8}$/);
