// Door station vendors (ADR 0026). To add a vendor: add its id to
// `DoorVendor` (API and database enum), add a guide module in `guides/` and
// list it here. The guide text keys use the prefix `doorGuide.<vendor>.`.
import type { DoorVendor } from '../api.ts';
import type { VendorGuide } from './guide.ts';
import { dahuaGuide } from './guides/dahua.ts';

export const vendorGuides: (VendorGuide & { id: DoorVendor })[] = [dahuaGuide];

/** Vendors for the door station form. */
export const doorVendors: { id: DoorVendor; name: string }[] = vendorGuides.map((v) => ({
	id: v.id,
	name: v.name
}));

export { commonSections } from './guides/common.ts';
