
const aircraft_btn_handler = async () => {
	let aircraft = random_mode_s();
	const request = await fetch(`https://api.adsbdb.com/v0/aircraft/${aircraft}`);
	const response = await request.json();
	document.querySelector("#random_aircraft").textContent = JSON.stringify(response, null, 2)
}

const random_aircraft_btn = document.getElementById("random_aircraft_btn");
random_aircraft_btn.addEventListener("click", aircraft_btn_handler);

const callsign_btn_handler = async () => {
	let callsign = random_callsign();
	const request = await fetch(`https://api.adsbdb.com/v0/callsign/${callsign}`);
	const response = await request.json();
	document.querySelector("#random_callsign").textContent = JSON.stringify(response, null, 2)
}

const random_callsign_btn = document.getElementById("random_callsign_btn");
random_callsign_btn.addEventListener("click", callsign_btn_handler);

const airline_btn_handler = async () => {
	let airline = random_airline();
	const request = await fetch(`https://api.adsbdb.com/v0/airline/${airline}`);
	const response = await request.json();
	document.querySelector("#random_airline").textContent = JSON.stringify(response, null, 2)
}

const random_airline_btn = document.getElementById("random_airline_btn");
random_airline_btn.addEventListener("click", airline_btn_handler);

const zeroPad = (unit) => String(unit).padStart(2, '0');

const secondsToText = (s) => {
	const second = zeroPad(Math.trunc(s % 60));
	const minute = zeroPad(Math.floor(s / 60 % 60));
	const hour = zeroPad(Math.floor(s / 60 / 60 % 24));
	const day = zeroPad(Math.floor(s / 60 / 60 / 24));
	return `${day} days, ${hour} hours, ${minute} minutes, ${second} seconds`;
};

const check_status = async () => {
	try {
		let ms = 0;
		const request = await fetch("https://api.adsbdb.com/v0/online");
		const response = await request.json();
		const uptime = document.querySelector("#uptime");
		const api_version = document.querySelector("#api_version");
		if (response?.response?.uptime && response?.response?.api_version) {
			api_version.innerHTML = response.response.api_version;
			ms = response.response.uptime;
			uptime.innerHTML = secondsToText(ms)
			document.querySelector('#stats').style.display = 'block';
			setInterval(() => {
				ms++;
				uptime.innerHTML = secondsToText(ms);
			}, 1000);
		}
	} catch (e) {
		console.log(e)
	}
}

check_status();

const random_mode_s = () => {
	const randomIndex = Math.floor(Math.random() * mode_s.length);
	return mode_s[randomIndex];
}

const random_callsign = () => {
	const randomIndex = Math.floor(Math.random() * callsign.length);
	return callsign[randomIndex];
}

const random_airline = () => {
	const randomIndex = Math.floor(Math.random() * airline.length);
	return airline[randomIndex];
}

const mode_s = [
	"0A4032",
	"0AC3E1",
	"0C20DF",
	"0D08A5",
	"152033",
	"300392",
	"380A95",
	"38133D",
	"3829A8",
	"383A38",
	"383A4A",
	"383D89",
	"390AF7",
	"39640B",
	"398031",
	"398495",
	"39B24B",
	"3D0590",
	"3D092D",
	"3D11D9",
	"3D2465",
	"3D25DB",
	"3D292F",
	"3D30FE",
	"3D325C",
	"3DE5D5",
	"3E67B1",
	"3E716D",
	"3E7EC3",
	"3EE270",
	"3EE67E",
	"3EEBCC",
	"3EF1C2",
	"3F29A3",
	"402412",
	"402950",
	"4059A5",
	"40622B",
	"4072A1",
	"44D1F9",
	"45CAA8",
	"473515",
	"473986",
	"49D30F",
	"4AD995",
	"4B0D23",
	"4B1ACB",
	"4C2C1E",
	"4CAFB3",
	"4D2101",
	"505D8B",
	"780A8A",
	"780DD6",
	"781C8A",
	"7BB0E8",
	"7C175B",
	"A0138A",
	"A039FC",
	"A18B41",
	"A19910",
	"A1B95F",
	"A1F87E",
	"A20010",
	"A242A8",
	"A330A0",
	"A36806",
	"A396E2",
	"A40CB2",
	"A46F24",
	"A4B958",
	"A5B10A",
	"A5D18E",
	"A67DCA",
	"A692BB",
	"A7036C",
	"A727F5",
	"A7A41D",
	"A8530F",
	"A9047D",
	"A915CF",
	"A97A36",
	"AA3AE6",
	"AA5F41",
	"AB2B94",
	"AB8FC9",
	"AB97CA",
	"AC042A",
	"AC8BB0",
	"AD6FF2",
	"AD8036",
	"ADDBA4",
	"ADF5C7",
	"ADFF4B",
	"AE4AE4",
	"AE6A1F",
	"C07E7A",
	"C8236E",
	"E03187",
	"E48B32",
	"E80447",
]
const callsign = [
	"AAL177",
	"AAL2251",
	"AAL2781",
	"ACA1330",
	"ACA430",
	"AFR90",
	"AMC306",
	"AMU882",
	"ANE8435",
	"ANZ65",
	"ATN4312",
	"BAW382",
	"BAW84PX",
	"BGH5555",
	"CAY608",
	"CCA732",
	"CFG1TL",
	"CLX710",
	"CND814",
	"CPA238",
	"CSN3378",
	"CYP467",
	"DAL1516",
	"DAL642",
	"DAL9925",
	"DHK38",
	"DLH18Y",
	"DLH66",
	"DLH8CN",
	"EDV4789",
	"EDV4862",
	"EDW78",
	"EIN1C6",
	"EJA811",
	"EJA853",
	"ELY16",
	"ELY25",
	"ENT535P",
	"ENT56EK",
	"ENT59UC",
	"EVA6606",
	"EWG465",
	"EXS47SR",
	"EXS878",
	"EZS2186",
	"EZY2146",
	"EZY278Z",
	"EZY34TZ",
	"EZY47UG",
	"FFT1093",
	"FFT329",
	"FFT8511",
	"FIN7052",
	"HFM543",
	"IBE841",
	"ISR49",
	"JAF21",
	"JBU2003",
	"JST712",
	"JZA595",
	"KLM168",
	"MXY139",
	"NKS1617",
	"OCN2013",
	"OHY6645",
	"PAC97",
	"PIA790",
	"QFA578",
	"RCH230",
	"RXA3881",
	"RXA4519",
	"RYR2AN",
	"RYR308D",
	"RYR4YX",
	"RYR6TA",
	"RYR91MQ",
	"RYR98BK",
	"SAS2002",
	"SAS2168",
	"SKW2900",
	"SKW4125",
	"SKW4742",
	"SVA2400",
	"SWA1082",
	"SWA9032",
	"SWA970",
	"TAI567",
	"TAP72",
	"TAP948N",
	"TAY59P",
	"THY3467",
	"TOM188",
	"TOM4541",
	"UAE7DK",
	"UAE978",
	"UAL904",
	"VKG456",
	"VLG3323",
	"VLG9GN",
	"WZZ5323",
];

const airline = [
 "AAO",
 "ADC",
 "AFV",
 "AJC",
 "ARP",
 "ASE",
 "AZU",
 "BFY",
 "BHR",
 "CBA",
 "CEV",
 "COH",
 "COV",
 "CRE",
 "CRW",
 "DCD",
 "DCF",
 "ECL",
 "EJD",
 "ELB",
 "FBZ",
 "FFS",
 "FNT",
 "FTN",
 "FWD",
 "FYA",
 "GRR",
 "HAK",
 "IDX",
 "ILW",
 "INU",
 "ISF",
 "ITU",
 "JAK",
 "JAV",
 "KFC",
 "KHA",
 "KMK",
 "KOQ",
 "LBR",
 "LEM",
 "LGN",
 "LYN",
 "MCY",
 "MMX",
 "MPC",
 "MPX",
 "MUS",
 "MWR",
 "MXB",
 "MYO",
 "NAI",
 "NLL",
 "NSM",
 "NST",
 "NVC",
 "NZM",
 "OLR",
 "PEA",
 "PNW",
 "PSL",
 "PTP",
 "RAZ",
 "RBE",
 "RBG",
 "RBT",
 "RGC",
 "RMX",
 "RPS",
 "SBA",
 "SFR",
 "SGY",
 "SKS",
 "SKX",
 "SOE",
 "SQA",
 "SSN",
 "STK",
 "SUR",
 "TBL",
 "TDV",
 "THZ",
 "TME",
 "TNC",
 "TNP",
 "TSH",
 "TSP",
 "UAL",
 "UHS",
 "USX",
 "VFC",
 "VFT",
 "VGA",
 "VKA",
 "WCP",
 "WTF",
 "XPL",
 "YOG",
 "ZZZ",
]