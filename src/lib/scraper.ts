import { CallSign, ICAO, ModeS, TAircraftPhoto, TScrapedIcao } from '../types';
import { log } from '../config/log';
import { } from '../const/env';
import { axios_service } from './axios';

const wrap = <T>() => function (_target: Scraper, propertyKey: string, descriptor: PropertyDescriptor): void {
	const original = descriptor.value;
	descriptor.value = async function (t: T): Promise<unknown> {
		const start = Date.now();
		try {
			const result = await original.call(this, t);
			return result;
		} catch (e) {
			log.error(e);
			return;
		} finally {
			log.debug(`scraper: ${propertyKey} wrap: ${Date.now()-start}ms`);
		}
	};
};

class Scraper {

	#convert_to_fullsize (x: string): string {
		return x ? x.replace('/thumbnails/', '/') : '';
	}

	#extract_icao (x: string): ICAO|undefined {
		const output = x.replace('"icao":', '').replace(/"/g, '');
		return output.match(/[A-Z]{3,4}/) ? <ICAO>output : undefined;
	}

	@wrap()
	async photo (mode_s: ModeS): Promise<TAircraftPhoto|undefined> {
		
		const response = await axios_service.get_photo(mode_s);
		if (!response || !response.data[0]?.image) return undefined;
		return {
			url_photo: this.#convert_to_fullsize(response.data[0]?.image),
			url_photo_thumbnail: response?.data[0].image,
			photographer: response?.data[0].photographer
		};
	}

	@wrap()
	async flightroute (callsign: CallSign): Promise<TScrapedIcao|undefined> {
		const data = await axios_service.get_flightroute(callsign);
		if (!data) return;
		const originIndex = data.search(/"icao":"[A-Z]{3,4}"/);
		if (originIndex < 0) return;
		const first_manipulation = data.substring(originIndex, originIndex + 1500);
		const origin_icao_string = first_manipulation.substring(0, 13);
		const second_manipulation = first_manipulation.replace(origin_icao_string, '');
		const destinationIndex = second_manipulation.search(/"icao":"[A-Z]{3,4}"/);
		if (destinationIndex < 0) return;
		const destination_icao_string = second_manipulation.substring(destinationIndex, destinationIndex +13);
		return {
			origin_icao: this.#extract_icao(origin_icao_string),
			destination_icao: this.#extract_icao(destination_icao_string)
		};
	}
}

export const scraper = new Scraper();