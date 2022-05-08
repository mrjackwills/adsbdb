import { CallSign, ModeS, TScraperPhoto } from '../types';
import { env } from '../const/env';
import Axios, { AxiosInstance, } from 'axios';

class AxiosService {

	#baseAxios!: AxiosInstance;

	constructor () {
		this.#baseAxios = Axios.create({
			headers: {
				'Accept': 'application/json',
				'Content-Type': 'application/json; charset=utf-8',
				'Cache-control': 'no-cache'
			},
		});
	}

	async get_flightroute (callsign: CallSign): Promise<string|undefined> {
		const { data } = await this.#baseAxios.get(`${env.URL_CALLSIGN}/${callsign}`, { timeout: 2500 });
		return typeof data === 'string' ? data : undefined;
	}
	
	async get_photo (mode_s: ModeS): Promise<TScraperPhoto|undefined> {
		const { data } = await this.#baseAxios.get(`${env.URL_AIRCRAFT_PHOTO}/ac_thumb.json?m=${mode_s}&n=1`, { timeout: 1000 });
		return data.status === 200 ? data : undefined;
	}
}

export const axios_service = new AxiosService();