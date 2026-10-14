// Setup guide for Dahua VTO door stations and VTH indoor monitors. Field
// names follow the Dahua interfaces (VTO web interface, ConfigTool); they
// differ between models and firmware versions.
import type { DoorVendor } from '../../api.ts';
import { raw, type GuideSection, type VendorGuide } from '../guide.ts';

const plan: GuideSection['table'] = {
	caption: 'doorGuide.dahua.prereq.plan',
	columns: ['doorGuide.colDevice', 'doorGuide.colExtension', 'doorGuide.colSipUser'],
	rows: [
		[raw('VTO'), raw('8000'), raw('8000vto')],
		[raw('VTH 1'), raw('8001'), raw('8001vth')],
		[raw('VTH 2'), raw('8002'), raw('8002vth')]
	]
};

const models: GuideSection = {
	id: 'models',
	title: 'doorGuide.dahua.models.title',
	table: {
		columns: ['doorGuide.colModel', 'doorGuide.colState'],
		rows: [
			[raw('VTO4202F-P-S2 · V4.600.0.3'), 'doorGuide.dahua.models.vto4202'],
			[raw('VTH5422H · V4.800.0.2'), 'doorGuide.dahua.models.vth5422'],
			[raw('VTO2202F, VTO2211G, VTO3221E'), 'doorGuide.dahua.models.other']
		]
	},
	unconfirmed: ['doorGuide.dahua.models.note']
};

const vto: GuideSection[] = [
	{
		id: 'prereq',
		title: 'doorGuide.dahua.prereq.title',
		steps: [
			'doorGuide.dahua.prereq.ownAccount',
			'doorGuide.dahua.prereq.separate',
			'doorGuide.dahua.prereq.network'
		],
		table: plan
	},
	{
		id: 'talkops',
		title: 'doorGuide.dahua.vto.talkops.title',
		steps: [
			'doorGuide.dahua.vto.talkops.ext',
			'doorGuide.dahua.vto.talkops.device',
			'doorGuide.dahua.talkops.format',
			'doorGuide.dahua.talkops.copy',
			'doorGuide.dahua.vto.talkops.station',
			'doorGuide.dahua.vto.talkops.target',
			'doorGuide.dahua.vto.talkops.test'
		]
	},
	{
		id: 'web',
		title: 'doorGuide.dahua.vto.web.title',
		intro: 'doorGuide.dahua.vto.web.intro',
		table: {
			columns: ['doorGuide.colField', 'doorGuide.colValue'],
			rows: [
				[raw('SIP Server (Enable)'), 'doorGuide.dahua.vto.web.enable'],
				[raw('Server Type'), 'doorGuide.dahua.vto.web.type'],
				[raw('IP Addr.'), 'doorGuide.dahua.web.host'],
				[raw('Port'), 'doorGuide.dahua.web.port'],
				[raw('Username + Enable'), 'doorGuide.dahua.vto.web.user'],
				[raw('Password'), 'doorGuide.dahua.web.password'],
				[raw('SIP Domain'), 'doorGuide.dahua.web.domain'],
				[raw('SIP Server Username'), 'doorGuide.dahua.vto.web.serverUser'],
				[raw('SIP Server Password'), 'doorGuide.dahua.vto.web.serverPassword']
			]
		},
		notes: [
			'doorGuide.dahua.vto.web.match',
			'doorGuide.dahua.vto.web.callNo',
			'doorGuide.dahua.vto.web.dtmf'
		]
	},
	{
		id: 'passwords',
		title: 'doorGuide.dahua.pw.title',
		table: {
			columns: ['doorGuide.colPassword', 'doorGuide.colWhere', 'doorGuide.colUse'],
			rows: [
				['doorGuide.pw.sip', 'doorGuide.dahua.vto.pw.sipWhere', 'doorGuide.pw.sipUse'],
				[
					'doorGuide.dahua.vto.pw.web',
					'doorGuide.dahua.vto.pw.webWhere',
					'doorGuide.dahua.vto.pw.webUse'
				]
			]
		},
		notes: ['doorGuide.dahua.pw.separate']
	},
	{
		id: 'check',
		title: 'doorGuide.dahua.check.title',
		steps: [
			'doorGuide.dahua.vto.check.save',
			'doorGuide.dahua.vto.check.restart',
			'doorGuide.dahua.vto.check.dashboard',
			'doorGuide.dahua.vto.check.ring'
		]
	},
	models
];

const vth: GuideSection[] = [
	{
		id: 'prereq',
		title: 'doorGuide.dahua.prereq.title',
		steps: [
			'doorGuide.dahua.vth.prereq.activate',
			'doorGuide.dahua.prereq.ownAccount',
			'doorGuide.dahua.vth.prereq.room',
			'doorGuide.dahua.prereq.separate',
			'doorGuide.dahua.prereq.network'
		],
		table: plan,
		unconfirmed: ['doorGuide.dahua.vth.prereq.limit']
	},
	{
		id: 'talkops',
		title: 'doorGuide.dahua.vto.talkops.title',
		steps: [
			'doorGuide.dahua.vth.talkops.ext',
			'doorGuide.dahua.vth.talkops.device',
			'doorGuide.dahua.talkops.format',
			'doorGuide.dahua.talkops.copy'
		],
		notes: ['doorGuide.dahua.vth.talkops.existing']
	},
	{
		id: 'configtool',
		title: 'doorGuide.dahua.vth.config.title',
		intro: 'doorGuide.dahua.vth.config.intro',
		table: {
			columns: ['doorGuide.colField', 'doorGuide.colValue'],
			rows: [
				[raw('Room No.'), 'doorGuide.dahua.vth.config.room'],
				[raw('VTH: Main VTH / Extension VTH'), 'doorGuide.dahua.vth.config.main'],
				[raw('Sip Server IP'), 'doorGuide.dahua.web.host'],
				[raw('Sip Server Port'), 'doorGuide.dahua.web.port'],
				[raw('Sip Register Pwd'), 'doorGuide.dahua.web.password'],
				[raw('Domain'), 'doorGuide.dahua.web.domain'],
				[raw('Initiative Mode'), 'doorGuide.dahua.vth.config.mode'],
				[raw('Login Password / Main VTH user'), 'doorGuide.dahua.vth.config.login']
			]
		},
		notes: ['doorGuide.dahua.vth.config.ok']
	},
	{
		id: 'screen',
		title: 'doorGuide.dahua.vth.screen.title',
		intro: 'doorGuide.dahua.vth.screen.intro',
		steps: ['doorGuide.dahua.vth.screen.one', 'doorGuide.dahua.vth.screen.same'],
		unconfirmed: ['doorGuide.dahua.vth.screen.unconfirmed']
	},
	{
		id: 'main',
		title: 'doorGuide.dahua.vth.main.title',
		steps: ['doorGuide.dahua.vth.main.own', 'doorGuide.dahua.vth.main.group'],
		notes: ['doorGuide.dahua.vth.main.ext'],
		unconfirmed: ['doorGuide.dahua.vth.main.unconfirmed']
	},
	{
		id: 'passwords',
		title: 'doorGuide.dahua.pw.title',
		table: {
			columns: ['doorGuide.colPassword', 'doorGuide.colWhere', 'doorGuide.colUse'],
			rows: [
				['doorGuide.pw.sip', 'doorGuide.dahua.vth.pw.sipWhere', 'doorGuide.pw.sipUse'],
				[
					'doorGuide.dahua.vth.pw.device',
					'doorGuide.dahua.vth.pw.deviceWhere',
					'doorGuide.dahua.vth.pw.deviceUse'
				],
				[
					'doorGuide.dahua.vth.pw.login',
					'doorGuide.dahua.vth.pw.loginWhere',
					'doorGuide.dahua.vth.pw.loginUse'
				]
			]
		},
		notes: ['doorGuide.dahua.pw.separate', 'doorGuide.dahua.vth.pw.mask']
	},
	{
		id: 'check',
		title: 'doorGuide.dahua.check.title',
		steps: [
			'doorGuide.dahua.vth.check.save',
			'doorGuide.dahua.vth.check.restart',
			'doorGuide.dahua.vth.check.dashboard',
			'doorGuide.dahua.vth.check.call',
			'doorGuide.dahua.vth.check.trouble'
		]
	},
	models
];

export const dahuaGuide: VendorGuide & { id: DoorVendor } = {
	id: 'dahua',
	name: 'Dahua',
	devices: [
		{
			id: 'vto',
			label: 'doorGuide.dahua.vto.label',
			summary: 'doorGuide.dahua.vto.summary',
			sections: vto
		},
		{
			id: 'vth',
			label: 'doorGuide.dahua.vth.label',
			summary: 'doorGuide.dahua.vth.summary',
			sections: vth
		}
	]
};
